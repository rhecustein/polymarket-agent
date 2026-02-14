pub mod gemini;
pub mod claude;

// Re-export for convenience (used by team modules)
#[allow(unused_imports)]
pub use gemini::GeminiClient;
#[allow(unused_imports)]
pub use claude::ClaudeClient;
