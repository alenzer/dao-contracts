use cosmwasm_std::{to_binary, Addr, Empty, Uint128, WasmMsg};
use cw2::ContractVersion;
use cw_governance_interface::voting::VotingPowerAtHeightResponse;
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use crate::{
    msg::{Admin, ExecuteMsg, InstantiateMsg, ModuleInstantiateInfo, QueryMsg},
    query::{DumpStateResponse, GetItemResponse},
    state::Config,
    ContractError,
};

const CREATOR_ADDR: &str = "creator";

fn cw20_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn sudo_govmod_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_govmod_sudo::contract::execute,
        cw_govmod_sudo::contract::instantiate,
        cw_govmod_sudo::contract::query,
    );
    Box::new(contract)
}

fn cw20_balances_voting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_balance_voting::contract::execute,
        cw20_balance_voting::contract::instantiate,
        cw20_balance_voting::contract::query,
    )
    .with_reply(cw20_balance_voting::contract::reply);
    Box::new(contract)
}

fn cw_gov_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply);
    Box::new(contract)
}

fn instantiate_gov(app: &mut App, code_id: u64, msg: InstantiateMsg) -> Addr {
    app.instantiate_contract(
        code_id,
        Addr::unchecked(CREATOR_ADDR),
        &msg,
        &[],
        "cw-governance",
        None,
    )
    .unwrap()
}

fn test_instantiate_with_n_gov_modules(n: usize) {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "DAO".to_string(),
        symbol: "DAO".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: None,
        marketing: None,
    };
    let instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_binary(&cw20_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: (0..n)
            .map(|n| ModuleInstantiateInfo {
                code_id: cw20_id,
                msg: to_binary(&cw20_instantiate).unwrap(),
                admin: Admin::GovernanceContract {},
                label: format!("governance module {}", n),
            })
            .collect(),
    };
    let gov_addr = instantiate_gov(&mut app, gov_id, instantiate);

    let state: DumpStateResponse = app
        .wrap()
        .query_wasm_smart(&gov_addr, &QueryMsg::DumpState {})
        .unwrap();

    assert_eq!(
        state.config,
        Config {
            name: "DAO DAO".to_string(),
            description: "A DAO that builds DAOs.".to_string(),
            image_url: None,
        }
    );

    assert_eq!(
        state.version,
        ContractVersion {
            contract: "crates.io:cw-governance".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string()
        }
    );

    assert_eq!(state.governance_modules.len(), n);
}

#[test]
#[should_panic(expected = "Execution would result in no governance modules being present.")]
fn test_instantiate_with_zero_gov_modules() {
    test_instantiate_with_n_gov_modules(0)
}

#[test]
fn test_valid_instantiate() {
    let module_counts = [1, 2, 200];
    for count in module_counts {
        test_instantiate_with_n_gov_modules(count)
    }
}

#[test]
#[should_panic(expected = "Error parsing into type cw20_base::msg::InstantiateMsg: Invalid type")]
fn test_instantiate_with_submessage_failure() {
    let mut app = App::default();
    let cw20_id = app.store_code(cw20_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let cw20_instantiate = cw20_base::msg::InstantiateMsg {
        name: "DAO".to_string(),
        symbol: "DAO".to_string(),
        decimals: 6,
        initial_balances: vec![],
        mint: None,
        marketing: None,
    };

    let mut governance_modules = (0..3)
        .map(|n| ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_binary(&cw20_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: format!("governance module {}", n),
        })
        .collect::<Vec<_>>();
    governance_modules.push(ModuleInstantiateInfo {
        code_id: cw20_id,
        msg: to_binary("bad").unwrap(),
        admin: Admin::GovernanceContract {},
        label: "I have a bad instantiate message".to_string(),
    });
    governance_modules.push(ModuleInstantiateInfo {
        code_id: cw20_id,
        msg: to_binary(&cw20_instantiate).unwrap(),
        admin: Admin::GovernanceContract {},
        label: "Everybody knowing
that goodness is good
makes wickedness."
            .to_string(),
    });

    let instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: cw20_id,
            msg: to_binary(&cw20_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: governance_modules,
    };
    instantiate_gov(&mut app, gov_id, instantiate);
}

#[test]
fn test_update_config() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    let modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            gov_addr.clone(),
            &QueryMsg::GovernanceModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(modules.len(), 1);

    let expected_config = Config {
        name: "Root DAO".to_string(),
        description: "We love trees and sudo.".to_string(),
        image_url: Some("https://moonphase.is/image.svg".to_string()),
    };

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        modules[0].clone(),
        &cw_govmod_sudo::msg::ExecuteMsg::Execute {
            msgs: vec![WasmMsg::Execute {
                contract_addr: gov_addr.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::UpdateConfig {
                    config: expected_config.clone(),
                })
                .unwrap(),
            }
            .into()],
        },
        &[],
    )
    .unwrap();

    let config: Config = app
        .wrap()
        .query_wasm_smart(gov_addr, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(expected_config, config)
}

fn test_swap_governance(swaps: Vec<(u64, u64)>) {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    let modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            gov_addr.clone(),
            &QueryMsg::GovernanceModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(modules.len(), 1);

    for (add, remove) in swaps {
        let start_modules: Vec<Addr> = app
            .wrap()
            .query_wasm_smart(
                gov_addr.clone(),
                &QueryMsg::GovernanceModules {
                    start_at: None,
                    limit: None,
                },
            )
            .unwrap();

        let to_add: Vec<_> = (0..add)
            .map(|n| ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_binary(&govmod_instantiate).unwrap(),
                admin: Admin::GovernanceContract {},
                label: format!("governance module {}", n),
            })
            .collect();

        let to_remove: Vec<_> = start_modules
            .iter()
            .take(remove as usize)
            .map(|a| a.to_string())
            .collect();

        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            start_modules[0].clone(),
            &cw_govmod_sudo::msg::ExecuteMsg::Execute {
                msgs: vec![WasmMsg::Execute {
                    contract_addr: gov_addr.to_string(),
                    funds: vec![],
                    msg: to_binary(&ExecuteMsg::UpdateGovernanceModules { to_add, to_remove })
                        .unwrap(),
                }
                .into()],
            },
            &[],
        )
        .unwrap();

        let finish_modules: Vec<Addr> = app
            .wrap()
            .query_wasm_smart(
                gov_addr.clone(),
                &QueryMsg::GovernanceModules {
                    start_at: None,
                    limit: None,
                },
            )
            .unwrap();

        assert_eq!(
            finish_modules.len() as u64,
            start_modules.len() as u64 + add - remove
        );
        for module in start_modules.into_iter().take(remove as usize) {
            assert!(!finish_modules.contains(&module))
        }
    }
}

#[test]
fn test_update_governance() {
    test_swap_governance(vec![(1, 1), (5, 0), (0, 5), (0, 0)])
}

#[test]
#[should_panic(expected = "Execution would result in no governance modules being present.")]
fn test_swap_governance_bad() {
    test_swap_governance(vec![(1, 1), (0, 1)])
}

#[test]
fn test_swap_voting_module() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    let voting_addr: Addr = app
        .wrap()
        .query_wasm_smart(gov_addr.clone(), &QueryMsg::VotingModule {})
        .unwrap();

    let modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            gov_addr.clone(),
            &QueryMsg::GovernanceModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(modules.len(), 1);

    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        modules[0].clone(),
        &cw_govmod_sudo::msg::ExecuteMsg::Execute {
            msgs: vec![WasmMsg::Execute {
                contract_addr: gov_addr.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::UpdateVotingModule {
                    module: ModuleInstantiateInfo {
                        code_id: govmod_id,
                        msg: to_binary(&govmod_instantiate).unwrap(),
                        admin: Admin::GovernanceContract {},
                        label: "voting module".to_string(),
                    },
                })
                .unwrap(),
            }
            .into()],
        },
        &[],
    )
    .unwrap();

    let new_voting_addr: Addr = app
        .wrap()
        .query_wasm_smart(gov_addr, &QueryMsg::VotingModule {})
        .unwrap();

    assert_ne!(new_voting_addr, voting_addr);
}

fn test_unauthorized(app: &mut App, gov_addr: Addr, msg: ExecuteMsg) {
    let err: ContractError = app
        .execute_contract(Addr::unchecked(CREATOR_ADDR), gov_addr, &msg, &[])
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_permissions() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let gov_id = app.store_code(cw_gov_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    test_unauthorized(
        &mut app,
        gov_addr.clone(),
        ExecuteMsg::UpdateVotingModule {
            module: ModuleInstantiateInfo {
                code_id: govmod_id,
                msg: to_binary(&govmod_instantiate).unwrap(),
                admin: Admin::GovernanceContract {},
                label: "voting module".to_string(),
            },
        },
    );

    test_unauthorized(
        &mut app,
        gov_addr.clone(),
        ExecuteMsg::UpdateGovernanceModules {
            to_add: vec![],
            to_remove: vec![],
        },
    );

    test_unauthorized(
        &mut app,
        gov_addr,
        ExecuteMsg::UpdateConfig {
            config: Config {
                name: "Evil config.".to_string(),
                description: "👿".to_string(),
                image_url: None,
            },
        },
    );
}

#[test]
fn test_passthrough_voting_queries() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let voting_id = app.store_code(cw20_balances_voting());
    let gov_id = app.store_code(cw_gov_contract());
    let cw20_id = app.store_code(cw20_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };
    let voting_instantiate = cw20_balance_voting::msg::InstantiateMsg {
        token_info: cw20_balance_voting::msg::TokenInfo::New {
            code_id: cw20_id,
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting_id,
            msg: to_binary(&voting_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    let creator_voting_power: VotingPowerAtHeightResponse = app
        .wrap()
        .query_wasm_smart(
            gov_addr,
            &QueryMsg::VotingPowerAtHeight {
                address: CREATOR_ADDR.to_string(),
                height: None,
            },
        )
        .unwrap();

    assert_eq!(
        creator_voting_power,
        VotingPowerAtHeightResponse {
            power: Uint128::from(2u64),
            height: app.block_info().height,
        }
    );
}

fn set_item(app: &mut App, gov_addr: Addr, key: String, addr: String) {
    app.execute_contract(
        gov_addr.clone(),
        gov_addr,
        &ExecuteMsg::SetItem { key, addr },
        &[],
    )
    .unwrap();
}

fn remove_item(app: &mut App, gov_addr: Addr, key: String) {
    app.execute_contract(
        gov_addr.clone(),
        gov_addr,
        &ExecuteMsg::RemoveItem { key },
        &[],
    )
    .unwrap();
}

fn get_item(app: &mut App, gov_addr: Addr, key: String) -> GetItemResponse {
    app.wrap()
        .query_wasm_smart(gov_addr, &QueryMsg::GetItem { key })
        .unwrap()
}

fn list_items(
    app: &mut App,
    gov_addr: Addr,
    start_at: Option<String>,
    limit: Option<u64>,
) -> Vec<Addr> {
    app.wrap()
        .query_wasm_smart(gov_addr, &QueryMsg::ListItems { start_at, limit })
        .unwrap()
}

#[test]
fn test_add_remove_get() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let voting_id = app.store_code(cw20_balances_voting());
    let gov_id = app.store_code(cw_gov_contract());
    let cw20_id = app.store_code(cw20_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };
    let voting_instantiate = cw20_balance_voting::msg::InstantiateMsg {
        token_info: cw20_balance_voting::msg::TokenInfo::New {
            code_id: cw20_id,
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting_id,
            msg: to_binary(&voting_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();
    let a = get_item(&mut app, gov_addr.clone(), "aaaaa".to_string());
    assert_eq!(a, GetItemResponse { item: None });

    set_item(
        &mut app,
        gov_addr.clone(),
        "aaaaa".to_string(),
        "aaaaa".to_string(),
    );
    let a = get_item(&mut app, gov_addr.clone(), "aaaaa".to_string());
    assert_eq!(
        a,
        GetItemResponse {
            item: Some(Addr::unchecked("aaaaa"))
        }
    );

    remove_item(&mut app, gov_addr.clone(), "a".to_string());
    let a = get_item(&mut app, gov_addr.clone(), "a".to_string());
    assert_eq!(a, GetItemResponse { item: None });

    remove_item(&mut app, gov_addr, "b".to_string());
}

#[test]
fn test_list_items() {
    let mut app = App::default();
    let govmod_id = app.store_code(sudo_govmod_contract());
    let voting_id = app.store_code(cw20_balances_voting());
    let gov_id = app.store_code(cw_gov_contract());
    let cw20_id = app.store_code(cw20_contract());

    let govmod_instantiate = cw_govmod_sudo::msg::InstantiateMsg {
        root: CREATOR_ADDR.to_string(),
    };
    let voting_instantiate = cw20_balance_voting::msg::InstantiateMsg {
        token_info: cw20_balance_voting::msg::TokenInfo::New {
            code_id: cw20_id,
            label: "DAO DAO voting".to_string(),
            name: "DAO DAO".to_string(),
            symbol: "DAO".to_string(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::from(2u64),
            }],
            marketing: None,
        },
    };

    let gov_instantiate = InstantiateMsg {
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs.".to_string(),
        image_url: None,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: voting_id,
            msg: to_binary(&voting_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "voting module".to_string(),
        },
        governance_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: govmod_id,
            msg: to_binary(&govmod_instantiate).unwrap(),
            admin: Admin::GovernanceContract {},
            label: "governance module".to_string(),
        }],
    };

    let gov_addr = app
        .instantiate_contract(
            gov_id,
            Addr::unchecked(CREATOR_ADDR),
            &gov_instantiate,
            &[],
            "cw-governance",
            None,
        )
        .unwrap();

    set_item(
        &mut app,
        gov_addr.clone(),
        "foo".to_string(),
        "foo".to_string(),
    );
    set_item(
        &mut app,
        gov_addr.clone(),
        "bar".to_string(),
        "bar".to_string(),
    );

    let first_item = list_items(&mut app, gov_addr.clone(), None, Some(1));
    assert_eq!(first_item.len(), 1);
    assert_eq!(first_item[0], "foo".to_string());

    let no_items = list_items(&mut app, gov_addr.clone(), None, Some(0));
    assert_eq!(no_items.len(), 0);

    // Items are retreived in decending order so asking for foo with
    // no limit ought to give us a single item.
    let second_item = list_items(&mut app, gov_addr, Some("foo".to_string()), None);
    assert_eq!(second_item.len(), 1);
    assert_eq!(second_item[0], "foo".to_string());
}