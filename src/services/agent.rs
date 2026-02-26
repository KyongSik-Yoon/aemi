/// Shared types for all agent backends (Claude, Gemini, etc.)

/// Streaming message types for real-time agent responses.
/// All agent backends convert their native stream events into this common enum.
#[derive(Debug, Clone)]
pub enum StreamMessage {
    /// Initialization - contains session_id
    Init { session_id: String },
    /// Text response chunk
    Text { content: String },
    /// Tool use started
    ToolUse { name: String, input: String },
    /// Tool execution result
    ToolResult { content: String, is_error: bool },
    /// Background task notification
    TaskNotification { _task_id: String, _status: String, summary: String },
    /// Completion
    Done { result: String, session_id: Option<String> },
    /// Error
    Error { message: String },
}

/// Token for cooperative cancellation of streaming requests.
/// Holds a flag and the child process PID so the caller can kill it externally.
pub struct CancelToken {
    pub cancelled: std::sync::atomic::AtomicBool,
    pub child_pid: std::sync::Mutex<Option<u32>>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: std::sync::atomic::AtomicBool::new(false),
            child_pid: std::sync::Mutex::new(None),
        }
    }
}

/// Common response type for non-streaming agent execution
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub success: bool,
    pub response: Option<String>,
    #[allow(dead_code)]
    pub session_id: Option<String>,
    pub error: Option<String>,
}
