use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

// Each render pass records one begin/end timestamp pair (2 x u64 = 16 bytes).
const MAX_PASSES: u32 = 64;
const PASS_BYTES: u64 = 16;
pub const GROUP_COUNT: usize = 7;

/// Render pass categories shown in the Performance dialog. Passes in the same group are summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPassGroup {
    Shadow,
    BasePass,
    Lighting,
    Sky,
    Bloom,
    Composite,
    LineGrid,
}

impl GpuPassGroup {
    pub const ALL: [GpuPassGroup; GROUP_COUNT] = [
        GpuPassGroup::Shadow,
        GpuPassGroup::BasePass,
        GpuPassGroup::Lighting,
        GpuPassGroup::Sky,
        GpuPassGroup::Bloom,
        GpuPassGroup::Composite,
        GpuPassGroup::LineGrid,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GpuPassGroup::Shadow => "Shadow",
            GpuPassGroup::BasePass => "Base pass",
            GpuPassGroup::Lighting => "Lighting",
            GpuPassGroup::Sky => "Sky",
            GpuPassGroup::Bloom => "Bloom",
            GpuPassGroup::Composite => "Composite",
            GpuPassGroup::LineGrid => "Line grid",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// GPU time per pass group in milliseconds. `None` means the group did not run in the measured frame.
#[derive(Clone, Copy)]
pub struct GpuTimings {
    pub group_ms: [Option<f64>; GROUP_COUNT],
    pub total_ms: f64,
}

/// Measures GPU time of render passes with timestamp queries.
/// Results are read back asynchronously, so they arrive a few frames after the measured frame.
/// When TIMESTAMP_QUERY is unsupported, every method is a no-op.
pub struct GpuProfiler {
    state: Option<ProfilerState>,
}

struct ProfilerState {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    timestamp_period_ns: f64,
    recording: Cell<bool>,
    pass_groups: RefCell<Vec<GpuPassGroup>>,
    scheduled_groups: RefCell<Option<Vec<GpuPassGroup>>>,
    readback: RefCell<Option<PendingReadback>>,
    latest: Cell<Option<GpuTimings>>,
}

struct PendingReadback {
    ready: Arc<Mutex<Option<bool>>>,
    groups: Vec<GpuPassGroup>,
    byte_len: u64,
}

impl GpuProfiler {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, supported: bool) -> Self {
        if !supported {
            return Self { state: None };
        }

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("GPU pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: MAX_PASSES * 2,
        });
        let buffer_size = MAX_PASSES as u64 * PASS_BYTES;
        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU timestamp resolve buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU timestamp staging buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            state: Some(ProfilerState {
                query_set,
                resolve_buffer,
                staging_buffer,
                timestamp_period_ns: queue.get_timestamp_period() as f64,
                recording: Cell::new(false),
                pass_groups: RefCell::new(Vec::new()),
                scheduled_groups: RefCell::new(None),
                readback: RefCell::new(None),
                latest: Cell::new(None),
            }),
        }
    }

    pub fn is_supported(&self) -> bool {
        self.state.is_some()
    }

    pub fn latest_timings(&self) -> Option<GpuTimings> {
        self.state.as_ref()?.latest.get()
    }

    /// Call at the start of a frame. Collects a finished readback and decides whether this frame is measured.
    pub fn begin_frame(&self) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        state.collect();
        state.pass_groups.borrow_mut().clear();
        state.recording.set(state.readback.borrow().is_none());
    }

    /// Returns timestamp writes for a render pass. Pass it to `timestamp_writes` of the render pass descriptor.
    pub fn timestamp_writes(
        &self,
        group: GpuPassGroup,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let state = self.state.as_ref()?;
        if !state.recording.get() {
            return None;
        }
        let mut pass_groups = state.pass_groups.borrow_mut();
        let index = pass_groups.len() as u32;
        if index >= MAX_PASSES {
            return None;
        }
        pass_groups.push(group);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &state.query_set,
            beginning_of_pass_write_index: Some(index * 2),
            end_of_pass_write_index: Some(index * 2 + 1),
        })
    }

    /// Call before finishing the frame's command encoder.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        if !state.recording.get() {
            return;
        }
        let pass_groups = state.pass_groups.borrow().clone();
        if pass_groups.is_empty() {
            return;
        }
        let count = pass_groups.len() as u32;
        encoder.resolve_query_set(&state.query_set, 0..count * 2, &state.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &state.resolve_buffer,
            0,
            &state.staging_buffer,
            0,
            count as u64 * PASS_BYTES,
        );
        *state.scheduled_groups.borrow_mut() = Some(pass_groups);
    }

    /// Call after submitting the frame's command encoder to start mapping the staging buffer.
    pub fn request_readback(&self) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let scheduled = state.scheduled_groups.borrow_mut().take();
        let Some(groups) = scheduled else {
            return;
        };
        let byte_len = groups.len() as u64 * PASS_BYTES;
        let ready = Arc::new(Mutex::new(None));
        let ready_in_callback = ready.clone();
        state
            .staging_buffer
            .slice(..byte_len)
            .map_async(wgpu::MapMode::Read, move |result| {
                *ready_in_callback.lock().unwrap() = Some(result.is_ok());
            });
        *state.readback.borrow_mut() = Some(PendingReadback {
            ready,
            groups,
            byte_len,
        });
    }
}

impl ProfilerState {
    fn collect(&self) {
        let mut readback = self.readback.borrow_mut();
        let Some(succeeded) = readback
            .as_ref()
            .and_then(|pending| *pending.ready.lock().unwrap())
        else {
            return;
        };
        let pending = readback.take().unwrap();
        if !succeeded {
            return;
        }

        let timings = {
            let view = self
                .staging_buffer
                .slice(..pending.byte_len)
                .get_mapped_range();
            let stamps: Vec<u64> = view
                .chunks_exact(8)
                .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
                .collect();

            let mut group_ms: [Option<f64>; GROUP_COUNT] = [None; GROUP_COUNT];
            let mut total_ms = 0.0;
            for (i, group) in pending.groups.iter().enumerate() {
                let begin = stamps[i * 2];
                let end = stamps[i * 2 + 1];
                let ms = end.saturating_sub(begin) as f64 * self.timestamp_period_ns / 1.0e6;
                let slot = &mut group_ms[group.index()];
                *slot = Some(slot.unwrap_or(0.0) + ms);
                total_ms += ms;
            }
            GpuTimings { group_ms, total_ms }
        };
        self.staging_buffer.unmap();
        self.latest.set(Some(timings));
    }
}
