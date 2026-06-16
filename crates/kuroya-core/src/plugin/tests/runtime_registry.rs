use super::*;

#[test]
fn plugin_runtime_registry_preserves_entry_capabilities_and_first_provider() {
    let first = PluginDescriptor {
        root: PathBuf::from("plugins/first"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "example.plugin".to_owned(),
            name: "Example".to_owned(),
            version: "0.1.0".to_owned(),
            entry: Some(PathBuf::from("plugins/first/plugin.wasm")),
            activation_events: vec![
                PluginActivationEvent::OnStartupFinished,
                PluginActivationEvent::OnStartupFinished,
            ],
            capabilities: PluginCapabilities {
                commands: true,
                languages: true,
                network: true,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions {
                commands: vec![PluginCommandContribution {
                    id: "example.run".to_owned(),
                    title: "Run".to_owned(),
                    category: None,
                }],
                languages: vec![PluginLanguageContribution {
                    id: "example-lang".to_owned(),
                    extensions: vec!["example".to_owned()],
                    aliases: Vec::new(),
                }],
                ..PluginContributions::default()
            },
        },
    };
    let duplicate = PluginDescriptor {
        root: PathBuf::from("plugins/duplicate"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "example.plugin".to_owned(),
            name: "Duplicate".to_owned(),
            version: "9.9.9".to_owned(),
            entry: Some(PathBuf::from("plugins/duplicate/plugin.wasm")),
            activation_events: Vec::new(),
            capabilities: PluginCapabilities::default(),
            contributes: PluginContributions::default(),
        },
    };

    let registry = PluginRuntimeRegistry::from_plugins(&[first, duplicate]);

    assert_eq!(registry.len(), 1);
    let runtime = registry.plugin("example.plugin").unwrap();
    assert_eq!(runtime.name, "Example");
    assert_eq!(
        runtime.command_entry(),
        Some(Path::new("plugins/first/plugin.wasm"))
    );
    assert_eq!(
        runtime.activation_events,
        vec![
            PluginActivationEvent::OnStartupFinished,
            PluginActivationEvent::OnCommand("example.run".to_owned()),
            PluginActivationEvent::OnLanguage("example-lang".to_owned()),
        ]
    );
    assert!(runtime.activates_on_startup());
    assert!(runtime.activates_on_command("example.run"));
    assert!(runtime.activates_on_language("example-lang"));
    assert!(runtime.capabilities.network);
    assert!(registry.plugin("missing.plugin").is_none());
}

#[test]
fn plugin_runtime_registry_indexes_activation_events_without_duplicates() {
    let command_plugin = PluginDescriptor {
        root: PathBuf::from("plugins/command"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "command.plugin".to_owned(),
            name: "Command".to_owned(),
            version: "0.1.0".to_owned(),
            entry: Some(PathBuf::from("plugins/command/plugin.wasm")),
            activation_events: vec![
                PluginActivationEvent::OnCommand("command.run".to_owned()),
                PluginActivationEvent::OnStartupFinished,
            ],
            capabilities: PluginCapabilities {
                commands: true,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions {
                commands: vec![PluginCommandContribution {
                    id: "command.run".to_owned(),
                    title: "Run".to_owned(),
                    category: None,
                }],
                ..PluginContributions::default()
            },
        },
    };
    let language_plugin = PluginDescriptor {
        root: PathBuf::from("plugins/language"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "language.plugin".to_owned(),
            name: "Language".to_owned(),
            version: "0.1.0".to_owned(),
            entry: None,
            activation_events: Vec::new(),
            capabilities: PluginCapabilities {
                languages: true,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions {
                languages: vec![PluginLanguageContribution {
                    id: "language-id".to_owned(),
                    extensions: vec!["lang".to_owned()],
                    aliases: Vec::new(),
                }],
                ..PluginContributions::default()
            },
        },
    };
    let any_plugin = PluginDescriptor {
        root: PathBuf::from("plugins/any"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "any.plugin".to_owned(),
            name: "Any".to_owned(),
            version: "0.1.0".to_owned(),
            entry: None,
            activation_events: vec![PluginActivationEvent::Any],
            capabilities: PluginCapabilities::default(),
            contributes: PluginContributions::default(),
        },
    };

    let registry =
        PluginRuntimeRegistry::from_plugins(&[command_plugin, language_plugin, any_plugin]);

    let command_runtime = registry.plugin("command.plugin").unwrap();
    assert_eq!(
        command_runtime.activation_events,
        vec![
            PluginActivationEvent::OnCommand("command.run".to_owned()),
            PluginActivationEvent::OnStartupFinished,
        ]
    );
    assert_eq!(
        registry
            .plugins_for_command("command.run")
            .into_iter()
            .map(|plugin| plugin.plugin_id.as_str())
            .collect::<Vec<_>>(),
        vec!["command.plugin", "any.plugin"]
    );
    assert_eq!(
        registry
            .plugins_for_language("language-id")
            .into_iter()
            .map(|plugin| plugin.plugin_id.as_str())
            .collect::<Vec<_>>(),
        vec!["language.plugin", "any.plugin"]
    );
    assert_eq!(
        registry
            .startup_plugins()
            .into_iter()
            .map(|plugin| plugin.plugin_id.as_str())
            .collect::<Vec<_>>(),
        vec!["command.plugin", "any.plugin"]
    );
    assert_eq!(
        registry
            .plugins_for_command("missing.command")
            .into_iter()
            .map(|plugin| plugin.plugin_id.as_str())
            .collect::<Vec<_>>(),
        vec!["any.plugin"]
    );
}

#[test]
fn plugin_runtime_registry_returns_any_activation_once_for_specific_trigger() {
    let plugin = PluginDescriptor {
        root: PathBuf::from("plugins/any-command"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "any-command.plugin".to_owned(),
            name: "Any Command".to_owned(),
            version: "0.1.0".to_owned(),
            entry: Some(PathBuf::from("plugins/any-command/plugin.wasm")),
            activation_events: vec![
                PluginActivationEvent::Any,
                PluginActivationEvent::OnCommand("any-command.run".to_owned()),
            ],
            capabilities: PluginCapabilities::default(),
            contributes: PluginContributions::default(),
        },
    };
    let registry = PluginRuntimeRegistry::from_plugins(&[plugin]);

    let command_plugins = registry.plugins_for_command("any-command.run");
    assert_eq!(command_plugins.len(), 1);
    assert_eq!(command_plugins[0].plugin_id, "any-command.plugin");

    let mut state = PluginActivationState::default();
    let activations = state.activate_command(&registry, "any-command.run");
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].plugin_id, "any-command.plugin");
    assert!(state.activate_startup(&registry).is_empty());
}

#[test]
fn plugin_runtime_registry_activation_iter_dedupes_unsorted_indexes() {
    let registry = PluginRuntimeRegistry {
        plugins: vec![
            PluginRuntimeRegistration {
                plugin_id: "first.plugin".to_owned(),
                name: "First".to_owned(),
                version: "0.1.0".to_owned(),
                root: PathBuf::from("plugins/first"),
                entry: None,
                activation_events: Vec::new(),
                capabilities: PluginCapabilities::default(),
            },
            PluginRuntimeRegistration {
                plugin_id: "second.plugin".to_owned(),
                name: "Second".to_owned(),
                version: "0.1.0".to_owned(),
                root: PathBuf::from("plugins/second"),
                entry: None,
                activation_events: Vec::new(),
                capabilities: PluginCapabilities::default(),
            },
        ],
        by_id: BTreeMap::new(),
        by_command: BTreeMap::new(),
        by_language: BTreeMap::new(),
        startup: Vec::new(),
        any: vec![1, 0, 1],
    };

    let plugin_ids = registry
        .activation_plugin_iter(&[1, 1, 0])
        .map(|plugin| plugin.plugin_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(plugin_ids, vec!["second.plugin", "first.plugin"]);
}

#[test]
fn plugin_activation_state_activates_matching_plugins_once() {
    let command_plugin = PluginDescriptor {
        root: PathBuf::from("plugins/command"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "command.plugin".to_owned(),
            name: "Command".to_owned(),
            version: "0.1.0".to_owned(),
            entry: Some(PathBuf::from("plugins/command/plugin.wasm")),
            activation_events: vec![PluginActivationEvent::OnCommand("command.run".to_owned())],
            capabilities: PluginCapabilities {
                commands: true,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions::default(),
        },
    };
    let startup_plugin = PluginDescriptor {
        root: PathBuf::from("plugins/startup"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "startup.plugin".to_owned(),
            name: "Startup".to_owned(),
            version: "0.1.0".to_owned(),
            entry: None,
            activation_events: vec![PluginActivationEvent::OnStartupFinished],
            capabilities: PluginCapabilities::default(),
            contributes: PluginContributions::default(),
        },
    };
    let registry = PluginRuntimeRegistry::from_plugins(&[command_plugin, startup_plugin]);
    let mut state = PluginActivationState::default();

    let command_activations = state.activate_command(&registry, "command.run");
    assert_eq!(command_activations.len(), 1);
    assert_eq!(command_activations[0].plugin_id, "command.plugin");
    assert_eq!(
        command_activations[0].trigger,
        PluginActivationTrigger::Command("command.run".to_owned())
    );
    assert!(state.is_active("command.plugin"));
    assert_eq!(state.active_count(), 1);

    assert!(state.activate_command(&registry, "command.run").is_empty());

    let startup_activations = state.activate_startup(&registry);
    assert_eq!(startup_activations.len(), 1);
    assert_eq!(startup_activations[0].plugin_id, "startup.plugin");
    assert_eq!(
        startup_activations[0].trigger,
        PluginActivationTrigger::Startup
    );
    assert_eq!(state.active_count(), 2);

    state.clear();
    assert_eq!(state.active_count(), 0);
}
