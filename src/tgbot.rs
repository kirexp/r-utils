#[cfg(feature="tgbot")]
pub mod bot_structs {
    use std::future::Future;
    use std::pin::Pin;
    use frankenstein::{Contact, DeleteMessageParams, Document, EditMessageResponse, EditMessageTextParams, Message, MethodResponse, SendDocumentParams, SendMessageParams, SetMyCommandsParams};
    use regex::Regex;
    use crate::base::GenericResult;

    pub struct BotCommand {
        command: String,
        pub action: Box<dyn Fn(i64, String) -> Pin<Box<dyn Future<Output=GenericResult<Option<StepExecutionResult>>> + Send>> + Send + Sync>,
    }

    impl BotCommand {
        pub fn new<TP: Into<String> + Send + 'static, F, Fut>(command: TP, action: F) -> Self
        where
            F: Fn(i64, String) -> Fut + Send + Sync + 'static,
            Fut: Future<Output=GenericResult<Option<StepExecutionResult>>> + Send + 'static,
        {
            BotCommand {
                command: command.into(),
                action: Box::new(move |chat_id, command| Box::pin(action(chat_id, command))),
            }
        }

        pub fn check_pattern(&self, pattern: &str) -> bool {
            let regex = Regex::new(self.command.clone().as_str()).unwrap();
            regex.is_match(pattern)
        }
    }

    #[derive(Debug)]
    pub struct StepExecutionResult {
        pub result: Vec<ExecutionParam>,
        pub chat_id: i64,
    }

    pub enum Step {
        WhatEver(StepExecutionResult),
        Finit(StepExecutionResult)
    }

    impl StepExecutionResult {
        pub fn one(chat_id: i64, result: ExecutionParam) -> Self {
            Self {
                result: vec![result],
                chat_id,
            }
        }

        pub fn many(chat_id: i64, results: Vec<ExecutionParam>) -> Self {
            Self {
                result: results,
                chat_id,
            }
        }
    }

    pub struct SendResult;
    impl Into<SendResult> for MethodResponse<Message> {
        fn into(self) -> SendResult {
            SendResult
        }
    }

    impl Into<SendResult> for MethodResponse<bool> {
        fn into(self) -> SendResult {
            SendResult
        }
    }

    impl Into<SendResult> for EditMessageResponse {
        fn into(self) -> SendResult {
            SendResult
        }
    }

    #[derive(Debug)]
    pub enum ExecutionParam {
        SendMessage(SendMessageParams),
        SendAndStoreMessage(SendMessageParams),
        SendMenu(SetMyCommandsParams),
        SendFile(SendDocumentParams),
        RemoveMessage(DeleteMessageParams),
        SendEditMessage(EditMessageTextParams),
    }

    #[derive(Debug, Clone)]
    pub struct MessageWrapper {
        pub chat_id: i64,
        pub m_text: Option<String>,
        pub contact: Option<Box<Contact>>,
        pub user_name: Option<String>,
        pub file_content: Option<Box<Document>>,
    }
}
