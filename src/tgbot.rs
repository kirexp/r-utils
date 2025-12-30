#[cfg(feature = "tgbot")]
pub mod bot_structs {
    use crate::base::base::{BotResult, GenericResult};
    use async_trait::async_trait;
    use frankenstein::{
        Contact, DeleteMessageParams, Document, EditMessageResponse, EditMessageTextParams,
        Message, MethodResponse, SendDocumentParams, SendMessageParams, SetMyCommandsParams,
    };
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use crate::tgbot::bot_processing::GlobalStateMachine;

    #[derive(Clone, Debug)]
    pub struct TemporaryMessage {
        pub message_id: i64,
        pub text: String,
    }

    #[async_trait]
    pub trait TemporaryMessageProvider: Send + Sync {
        async fn store_message(
            &self,
            chat_id: i64,
            message: String,
            message_id: i64,
        ) -> BotResult<()>;
        async fn get_message(&self, chat_id: i64) -> Option<TemporaryMessage>;
        async fn remove_message(&self, chat_id: i64, message_id: i64) -> BotResult<()>;
    }

    pub struct BotCommand {
        command: String,
        pub action: Box<
            dyn Fn(
                    i64,
                    String,
                )
                    -> Pin<Box<dyn Future<Output = BotResult<Option<StepExecutionResult>>> + Send>>
                + Send
                + Sync,
        >,
    }

    impl BotCommand {
        pub fn new<TP: Into<String> + Send + 'static, F, Fut>(command: TP, action: F) -> Self
        where
            F: Fn(i64, String) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = BotResult<Option<StepExecutionResult>>> + Send + 'static,
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
        Finit(StepExecutionResult),
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

    #[derive(Serialize, Deserialize, Debug)]
    pub struct UserInfo {
        pub phone: String,
        pub chat_id: i64,
        pub user_name: String,
    }
    impl UserInfo {
        pub fn new(phone: String, chat_id: i64, user_name: String) -> Self {
            Self {
                phone,
                chat_id,
                user_name,
            }
        }
    }

    pub trait GetStateMachineName {
        fn get_name(&self) -> &'static str;
        fn get_name_st() -> &'static str
        where
            Self: Sized;
    }

    #[async_trait]
    pub trait ProcessNext {
        async fn process_next(
            &mut self,
            chat_id: i64,
            message: Option<String>,
            temp_message_processor: &GlobalStateMachine,
        ) -> BotResult<Step>;
    }

    #[async_trait]
    pub trait ProcessStateMachine:
        GetStateMachineName + ProcessNext + FinishStateMachine + Send
    {
    }

    #[async_trait]
    pub trait FinishStateMachine {
        async fn finish(&self) -> BotResult<()>;
    }
}

#[cfg(feature = "tgbot")]
pub mod bot_processing {

    use crate::base::base::{BotError, BotResult, GenericError, GenericResult};
    use crate::tgbot::bot_structs::{
        BotCommand, ExecutionParam, MessageWrapper, ProcessStateMachine, SendResult, Step,
        StepExecutionResult, TemporaryMessageProvider, UserInfo,
    };
    use async_trait::async_trait;
    use frankenstein::{
        BotCommand as BotMCommand, BotCommandScope, BotCommandScopeChat, CallbackQuery,
        KeyboardButton, Message, ReplyKeyboardMarkup, ReplyMarkup, SendMessageParams,
        SetMyCommandsParams,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Once, ONCE_INIT};
    use tokio::sync::{Mutex, RwLock, RwLockReadGuard};
    use tracing::info;

    static mut SINGLETON_INSTANCE: Option<StateMachineRepo> = None;
    static ONCE: Once = ONCE_INIT;
    pub struct StateMachineRepo {
        pub state_machines:
            Arc<RwLock<HashMap<i64, Arc<Mutex<dyn ProcessStateMachine + Send + Sync>>>>>,
    }

    impl StateMachineRepo {
        fn new() -> Self {
            Self {
                state_machines: Arc::new(RwLock::new(HashMap::new())),
            }
        }
        pub fn get_instance() -> &'static StateMachineRepo {
            unsafe {
                ONCE.call_once(|| {
                    SINGLETON_INSTANCE = Some(StateMachineRepo::new());
                });
                // SAFETY: The unsafe block ensures the lifetime of the reference is correct.
                SINGLETON_INSTANCE.as_ref().unwrap()
            }
        }

        pub async fn check_active_task_sm(
            &self,
            chat_id: i64,
        ) -> Option<Arc<Mutex<dyn ProcessStateMachine + Send + Sync>>> {
            let state_machines_read = self.state_machines.read().await;
            let potential_sm = state_machines_read.get(&chat_id)?;
            Some(Arc::clone(potential_sm))
        }

        pub async fn init_state_machine(
            &self,
            chat_id: i64,
            state_machine: impl ProcessStateMachine + Send + Sync + 'static,
        ) -> BotResult<()> {
            let new_sm: Arc<Mutex<dyn ProcessStateMachine + Send + Sync>> =
                Arc::new(Mutex::new(state_machine));
            let mut write_state = self.state_machines.write().await;
            write_state.insert(chat_id, new_sm);
            Ok(())
        }

        pub async fn complete_state_machine(&self, chat_id: i64) -> BotResult<()> {
            let mut write_state = self.state_machines.write().await;
            write_state.remove(&chat_id);
            Ok(())
        }
    }

    pub struct GlobalStateMachine {
        pub auth_processor: Arc<dyn AuthenticationProcessor>,
        pub temp_message_processor: Arc<dyn TemporaryMessageProvider>,
        pub commands: Vec<BotCommand>,
        pub api: Arc<AsyncApi>
    }

    impl GlobalStateMachine {
        pub fn new(
            auth_processor: Arc<dyn AuthenticationProcessor>,
            temp_message_processor: Arc<dyn TemporaryMessageProvider>,
            commands: Vec<BotCommand>,
            api: Arc<AsyncApi>
        ) -> Self {
            Self {
                auth_processor,
                temp_message_processor,
                commands,
                api,
            }
        }

        pub async fn process_message(
            &self,
            msg: Option<Message>,
            cq: Option<CallbackQuery>,
        ) -> BotResult<StepExecutionResult> {
            match (msg, cq) {
                (Some(msg), _) => {
                    let file_content = msg.document;
                    let p = MessageWrapper {
                        m_text: msg.text,
                        chat_id: msg.chat.id,
                        contact: msg.contact,
                        user_name: msg.chat.username,
                        file_content,
                    };
                    Ok(self.process_message_sm(p).await?)
                }
                (_, Some(cq)) => {
                    let p = MessageWrapper {
                        m_text: cq.data,
                        chat_id: cq.from.id as i64,
                        contact: None,
                        user_name: cq.from.username,
                        file_content: None,
                    };
                    Ok(self.process_message_sm(p).await?)
                }
                _ => {
                    panic!()
                }
            }
        }

        async fn process_message_sm(
            &self,
            message_to_process: MessageWrapper,
        ) -> BotResult<StepExecutionResult> {
            let chat_id = message_to_process.chat_id;
            let text = message_to_process.m_text.clone().unwrap_or(String::new());
            let sm_repo = StateMachineRepo::get_instance();
            let has_session_info = self.auth_processor.has_user(&chat_id).await;
            if !has_session_info && message_to_process.contact == None {
                let message_params = GetContactStep::execute(chat_id)?;
                return Ok(StepExecutionResult::one(
                    chat_id,
                    ExecutionParam::SendMessage(message_params),
                ));
            }
            if let Some(value) = self
                .auth_processor
                .process(&message_to_process, chat_id.clone())
                .await
            {
                return value;
            }
            let message_text = message_to_process.m_text.clone();
            if (message_text.is_some() && message_text.unwrap().to_lowercase() == "/cancel") {
                sm_repo.complete_state_machine(chat_id).await?;
                let send_message_params = SendMessageParams::builder()
                    .chat_id(chat_id)
                    .text("Отмена процесса добавления/удаления")
                    .build();
                return Ok(StepExecutionResult::many(
                    chat_id,
                    vec![ExecutionParam::SendMessage(send_message_params)],
                ));
            }

            if let Some(mut active_sm) = sm_repo.check_active_task_sm(chat_id).await {
                let mut active_sm = active_sm.lock().await;
                let sm_result = active_sm
                    .process_next(
                        chat_id,
                        message_to_process.m_text.clone(),
                        &self
                    )
                    .await
                    .map_err(|e| BotError::UnknownError(chat_id, e.to_string()))?;

                let sm_step_data = match sm_result {
                    Step::Finit(result) => {
                        active_sm.finish().await?;
                        sm_repo.complete_state_machine(chat_id).await?;
                        result
                    }
                    Step::WhatEver(result) => result,
                };

                return Ok(sm_step_data);
            }
            return match self.process_bot_commands(chat_id, &text).await? {
                Some(execution_result) => Ok(execution_result),
                None => {
                    match self
                        .process_other_messages(chat_id, &message_to_process)
                        .await
                    {
                        Some(step_exec_result) => Ok(step_exec_result),
                        None => {
                            let not_implemented_params = SendMessageParams::builder()
                                .chat_id(message_to_process.chat_id)
                                .text("Не реализовано!")
                                .build();
                            Ok(StepExecutionResult::one(
                                chat_id,
                                ExecutionParam::SendMessage(not_implemented_params),
                            ))
                        }
                    }
                }
            };
        }

        async fn process_other_messages(
            &self,
            chat_id: i64,
            message: &MessageWrapper,
        ) -> Option<StepExecutionResult> {
            None
        }

        async fn process_bot_commands(
            &self,
            chat_id: i64,
            command: &String,
        ) -> BotResult<Option<StepExecutionResult>> {
            for bot_command in &self.commands {
                let command_arc = Arc::new(command.clone());
                if bot_command.check_pattern(command_arc.as_str()) {
                    info!("Bot command was identified as {command_arc}. Starting execution");
                    let result = (bot_command.action)(chat_id, command.clone()).await?;
                    return Ok(result);
                }
            }
            Err(BotError::BusinessError(chat_id, "Command not found".to_string()))
        }
    }

    pub struct GetContactStep;

    impl GetContactStep {
        pub fn execute(chat_id: i64) -> BotResult<SendMessageParams> {
            let mut keyboard: Vec<Vec<KeyboardButton>> = Vec::new();
            let mut vek: Vec<KeyboardButton> = Vec::new();

            vek.push(
                KeyboardButton::builder()
                    .text("Поделиться контактом")
                    .request_contact(true)
                    .build(),
            );
            keyboard.push(vek);
            let keyboard_markup = ReplyKeyboardMarkup::builder()
                .keyboard(keyboard)
                .resize_keyboard(true)
                .one_time_keyboard(true)
                .build();

            let send_message_params = SendMessageParams::builder()
                .chat_id(chat_id)
                .text("Для продолжения, Вам нужно поделиться с ботом контактными данными!")
                .reply_markup(ReplyMarkup::ReplyKeyboardMarkup(keyboard_markup))
                .build();
            return Ok(send_message_params);
        }
    }

    #[async_trait]
    pub trait AuthenticationProcessor: Sync + Send {
        async fn process(
            &self,
            message_to_process: &MessageWrapper,
            chat_id: i64,
        ) -> Option<BotResult<StepExecutionResult>>;

        async fn has_user(&self, user_id: &i64) -> bool;
    }

    use frankenstein::{AsyncApi, AsyncTelegramApi, DeleteMyCommandsParams};
    use tracing::error;

    pub fn process_message(
        api: Arc<AsyncApi>,
        state: &Arc<GlobalStateMachine>,
        message: Option<Message>,
        callback_query: Option<CallbackQuery>,
    ) -> Result<SendResult, GenericError> {
        let api_clone = api;
        let sm = Arc::clone(&state);
        {
            let cloned_state = state.clone();
            tokio::spawn(async move {
                let cloned_state = cloned_state.clone();
                let step_execution_result = sm.process_message(message, callback_query).await;
                match step_execution_result {
                    Ok(result) => {
                        for execution_param in result.result {
                            let send_result = send_message(
                                api_clone.clone(),
                                &cloned_state,
                                result.chat_id,
                                &execution_param,
                            )
                            .await;
                            if let Err(error) = send_result {
                                error!("Failed to send message: {error:?}");
                            }
                        }
                    }
                    Err(err) => match err {
                        BotError::UnknownError(chat_id, error_text)
                        | BotError::BusinessError(chat_id, error_text) => {
                            let error_response = ExecutionParam::SendMessage(
                                SendMessageParams::builder()
                                    .chat_id(chat_id.clone())
                                    .text(error_text)
                                    .build(),
                            );
                            let send_result =
                                send_message(api_clone.clone(), &cloned_state, chat_id, &error_response)
                                    .await
                                    .expect("SendMessage failed");
                        }
                        BotError::ChatlessError(_) => panic!("Chatless error!"),
                    },
                }
            });
            return Ok(SendResult);
        }
    }

    async fn send_message(
        api_clone: Arc<AsyncApi>,
        cloned_state: &Arc<GlobalStateMachine>,
        chat_id: i64,
        execution_param: &ExecutionParam,
    ) -> Result<SendResult, GenericError> {
        let result: SendResult = match execution_param {
            ExecutionParam::SendMessage(message_params) => {
                api_clone.send_message(&message_params).await?.into()
            }
            ExecutionParam::SendAndStoreMessage(message_params) => {
                let result = api_clone.send_message(&message_params).await?;
                let cloned_state = cloned_state.clone();
                cloned_state
                    .temp_message_processor
                    .store_message(
                        chat_id,
                        message_params.text.to_string(),
                        result.result.message_id as i64,
                    )
                    .await?;
                result.into()
            }
            ExecutionParam::SendMenu(menu_params) => {
                let delete_params = DeleteMyCommandsParams::builder()
                    .scope(BotCommandScope::Chat(
                        BotCommandScopeChat::builder().chat_id(chat_id).build(),
                    ))
                    .build();
                api_clone.delete_my_commands(&delete_params).await?;
                api_clone.set_my_commands(&menu_params).await?.into()
            }
            ExecutionParam::SendFile(file_params) => {
                api_clone.send_document(&file_params).await?.into()
            }
            ExecutionParam::RemoveMessage(remove_message) => {
                api_clone.delete_message(&remove_message).await?.into()
            }
            ExecutionParam::SendEditMessage(edit_message) => {
                api_clone.edit_message_text(&edit_message).await?.into()
            }
        };

        return Ok(result);
    }

    #[cfg(test)]
    mod tests {
        use crate::base::base::{BotResult};
        use crate::tgbot::bot_processing::AuthenticationProcessor;
        use crate::tgbot::bot_structs::{MessageWrapper, StepExecutionResult};
        use async_trait::async_trait;

        pub struct AuthProcessor;

        #[async_trait]
        impl AuthenticationProcessor for AuthProcessor {
            async fn process(
                &self,
                message_to_process: &MessageWrapper,
                chat_id: i64,
            ) -> Option<BotResult<StepExecutionResult>> {
                todo!()
            }

            async fn has_user(&self, user_id: &i64) -> bool {
                false
            }
        }
    }
}

#[cfg(feature = "tgbot")]
pub use bot_macros::*;
