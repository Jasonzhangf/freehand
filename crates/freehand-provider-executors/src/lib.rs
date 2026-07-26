//! Production provider-adapter assembly for Freehand live executor factories.

use freehand_provider_anthropic::AnthropicExecutorFactory;
use freehand_provider_core::{
    ProviderExecutorConfig, ProviderExecutorFactory, ProviderExecutorFactoryError, ProviderFamily,
    ProviderLiveExecutor, ProviderProtocol,
};
use freehand_provider_openai::OpenAiExecutorFactory;

#[derive(Debug, Clone, Default)]
pub struct CompositeProviderExecutorFactory {
    openai: OpenAiExecutorFactory,
    anthropic: AnthropicExecutorFactory,
}

impl CompositeProviderExecutorFactory {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProviderExecutorFactory for CompositeProviderExecutorFactory {
    fn build_executor(
        &self,
        config: ProviderExecutorConfig,
    ) -> Result<Box<dyn ProviderLiveExecutor>, ProviderExecutorFactoryError> {
        match (config.descriptor.family, config.descriptor.protocol) {
            (ProviderFamily::OpenAiCompatible, ProviderProtocol::OpenAiResponses)
            | (ProviderFamily::OpenAiCompatible, ProviderProtocol::OpenAiChatCompletions) => {
                self.openai.build_executor(config)
            }
            (ProviderFamily::Anthropic, ProviderProtocol::AnthropicMessages) => {
                self.anthropic.build_executor(config)
            }
            (family, protocol) => {
                Err(ProviderExecutorFactoryError::Unsupported { family, protocol })
            }
        }
    }
}

pub fn production_provider_executor_factory() -> CompositeProviderExecutorFactory {
    CompositeProviderExecutorFactory::new()
}
