#![allow(improper_ctypes)]

mod data;
mod defaults;
mod types;

use defaults::*;
use marine_rs_sdk::marine;
use marine_rs_sdk::module_manifest;
use marine_rs_sdk::MountedBinaryResult;
use marine_rs_sdk::WasmLoggerBuilder;
use types::{FinalMetadata, MetaContract, MetaContractResult, Metadata, Transaction, TxParam};

module_manifest!();

pub fn main() {
    WasmLoggerBuilder::new()
        .with_log_level(log::LevelFilter::Info)
        .build()
        .unwrap();
}

#[marine]
pub fn on_execute(
    contract: MetaContract,
    metadatas: Vec<Metadata>,
    transaction: Transaction,
) -> MetaContractResult {
    let mut finals: Vec<FinalMetadata> = vec![];

    if !is_token_owner(transaction.public_key.clone(), transaction.token_id.clone()) {
      return MetaContractResult {
        result: false,
        metadatas: Vec::new(),
        error_string: "Invalid token owner".to_string(),
      }
    } 

    let serde_metadata: Result<Vec<String>, serde_json::Error> =
        serde_json::from_str(&transaction.data.clone());

    match serde_metadata {
        Ok(tx_data) => {
            let is_valid = tx_data
                .clone()
                .into_iter()
                .map(verify_cid)
                .all(|str| is_ipfs_cid(&str));

            if !is_valid {
                return MetaContractResult {
                    result: false,
                    metadatas: Vec::new(),
                    error_string: "Invalid cid provided".to_string(),
                };
            }

            let content = serde_json::to_string(&tx_data).unwrap();

            finals.push(FinalMetadata {
                public_key: transaction.public_key,
                alias: "".to_string(),
                content,
                version: transaction.version,
                loose: 0,
            });
        }
        Err(_) => {
            return MetaContractResult {
                result: false,
                metadatas: Vec::new(),
                error_string: "Data does not follow the required JSON schema".to_string(),
            }
        }
    }

    MetaContractResult {
        result: true,
        metadatas: finals,
        error_string: "".to_string(),
    }
}

#[marine]
pub fn on_clone() -> bool {
    return false;
}

#[marine]
pub fn on_mint(
    contract: MetaContract,
    data_key: String,
    token_id: String,
    data: String,
) -> MetaContractResult {
    MetaContractResult {
        result: false,
        metadatas: vec![],
        error_string: "on_mint is not available".to_string(),
    }
}
/**
 * Get data from ipfs
 */
fn get(hash: String, api_multiaddr: String, timeout_sec: u64) -> String {
    let address: String;
    let t;

    if api_multiaddr.is_empty() {
        address = DEFAULT_IPFS_MULTIADDR.to_string();
    } else {
        address = api_multiaddr;
    }

    if timeout_sec == 0 {
        t = DEFAULT_TIMEOUT_SEC;
    } else {
        t = timeout_sec;
    }

    let args = vec![String::from("dag"), String::from("get"), hash];

    let cmd = make_cmd_args(args, address, t);

    let result = ipfs(cmd);

    String::from_utf8(result.stdout).unwrap()
}

pub fn make_cmd_args(args: Vec<String>, api_multiaddr: String, timeout_sec: u64) -> Vec<String> {
    args.into_iter()
        .chain(vec![
            String::from("--timeout"),
            get_timeout_string(timeout_sec),
            String::from("--api"),
            api_multiaddr,
        ])
        .collect()
}

#[inline]
pub fn get_timeout_string(timeout: u64) -> String {
    format!("{}s", timeout)
}

fn get_token_owner(token_id: String) -> String {
  let mut tx_params: Vec<TxParam> = vec![];

  tx_params.push(TxParam {
    value_type: "uint".to_string(),
    value: token_id.clone()
  });

  let token_owner = evm_read_contract(
    DEFAULT_NODE_URL.to_string(), 
    DEFAULT_ABI_URL.to_string(), 
    "ownerOf".to_string(), 
    DEFAULT_CONTRACT_ADDRESS.to_string(),
    tx_params,
  );

  evm_shorten_hex(token_owner, 40)
}

fn is_token_owner(owner: String, token_id: String) -> bool {
  let token_owner = get_token_owner(token_id.clone());

  token_owner.to_lowercase() == owner.to_lowercase() 
}

// Service
// - curl

#[marine]
#[link(wasm_import_module = "host")]
extern "C" {
    pub fn ipfs(cmd: Vec<String>) -> MountedBinaryResult;
}

#[marine]
#[link(wasm_import_module = "evm_rpc")]
extern "C" {
    #[link_name = "contract_view_call"]
    pub fn evm_read_contract(
        node_url: String,
        abi_url: String,
        method_name: String,
        contract_address: String,
        tx_params: Vec<TxParam>,
    ) -> String;

    #[link_name = "shorten_hex"]
    pub fn evm_shorten_hex(hex: String, to_len: u32) -> String;
}

/**
 * For now leaving it empty. Freedom of speech
 */
pub fn is_profane(text: &str) -> bool {
    let profane_words = vec!["", ""];
    profane_words.iter().any(|&word| {
        if word != "" {
            return text.contains(word);
        }
        false
    })
}

pub fn is_nft_storage_link(link: &str) -> bool {
    link == "" || link.starts_with("https://nftstorage.link/ipfs/")
}

fn is_ipfs_cid(cid: &str) -> bool {
    cid == "" || cid.starts_with("/ipfs/")
}

fn verify_cid(hash: String) -> String {
    let result = ipfs(["resolve".to_string(), hash.to_string()].to_vec());

    match String::from_utf8(result.stdout) {
        Ok(url) => {
            if url.is_empty() {
                return String::from("/");
            };
            return url;
        }

        Err(_) => return String::from("Something went wrong"),
    }
}
