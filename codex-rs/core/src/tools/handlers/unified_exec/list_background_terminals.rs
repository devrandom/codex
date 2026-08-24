use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::super::shell_spec::create_list_background_terminals_tool;

pub struct ListBackgroundTerminalsHandler;

const NO_BACKGROUND_TERMINALS_MESSAGE: &str =
    "No background terminal sessions are currently running.";

impl ToolExecutor<ToolInvocation> for ListBackgroundTerminalsHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_background_terminals")
    }

    fn spec(&self) -> ToolSpec {
        create_list_background_terminals_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl ListBackgroundTerminalsHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        if !matches!(payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::RespondToModel(
                "list_background_terminals handler received unsupported payload".to_string(),
            ));
        }

        let terminals = session.services.unified_exec_manager.list_processes().await;

        let text = if terminals.is_empty() {
            NO_BACKGROUND_TERMINALS_MESSAGE.to_string()
        } else {
            terminals
                .iter()
                .map(|terminal| format!("{}: {}", terminal.process_id, terminal.command))
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            text,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for ListBackgroundTerminalsHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}
