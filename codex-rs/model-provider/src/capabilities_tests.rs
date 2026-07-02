use super::*;
use crate::create_model_provider;
use codex_model_provider_info::ModelProviderCapabilities;
use pretty_assertions::assert_eq;

#[test]
fn capability_overrides_preserve_unspecified_provider_defaults() {
    for info in [
        ModelProviderInfo::default(),
        ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        ModelProviderInfo {
            name: "Azure".to_string(),
            ..Default::default()
        },
    ] {
        let defaults = create_model_provider(info.clone(), /*auth_manager*/ None).capabilities();
        let provider = create_model_provider(
            ModelProviderInfo {
                capabilities: Some(ModelProviderCapabilities {
                    external_web_access: Some(false),
                    ..Default::default()
                }),
                ..info
            },
            /*auth_manager*/ None,
        );
        assert_eq!(
            provider.capabilities(),
            ProviderCapabilities {
                external_web_access: false,
                ..defaults
            },
        );
    }
}

#[test]
fn custom_provider_disables_hosted_and_namespace_tools() {
    let provider = create_model_provider(
        ModelProviderInfo {
            name: "custom".to_string(),
            base_url: Some("https://example.com/v1".to_string()),
            requires_openai_auth: false,
            ..Default::default()
        },
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.capabilities(),
        ProviderCapabilities {
            namespace_tools: false,
            image_generation: false,
            web_search: false,
            external_web_access: false,
            ..ProviderCapabilities::default()
        },
    );
}

#[test]
fn openai_provider_keeps_hosted_and_namespace_tools() {
    let provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider.capabilities(),
        ProviderCapabilities {
            remote_compaction: RemoteCompactionSupport::V2,
            ..ProviderCapabilities::default()
        },
    );
}
