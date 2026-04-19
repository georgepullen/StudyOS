pub mod app;
mod protocol;
pub mod runtime;
pub mod tui;

pub use app::{
    App, AppAction, AppBootstrap, FocusRegion, tutor_close_output_schema, tutor_output_schema,
    tutor_submission_output_schema,
};
pub use runtime::{
    AppServerTransport, CodexAppServerTransport, RecordedServerLine, ReplayAppServerTransport,
    RuntimeEvent, capture_runtime_fixture,
};
