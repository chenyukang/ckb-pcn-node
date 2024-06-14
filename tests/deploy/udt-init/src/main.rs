use ckb_sdk::{
    transaction::{
        builder::{sudt::SudtTransactionBuilder, CkbTransactionBuilder},
        handler::{sighash::Secp256k1Blake160SighashAllScriptHandler, sudt::SudtHandler},
        input::InputIterator,
        signer::{SignContexts, TransactionSigner},
        TransactionBuilderConfiguration,
    },
    Address, CkbRpcClient, NetworkInfo, ScriptId,
};
use ckb_types::{
    core::{DepType, ScriptHashType},
    h256,
    packed::{OutPoint, Script},
    prelude::{Entity, Pack},
    H256,
};
use ckb_types::{packed::CellDep, prelude::Builder};

use std::{error::Error as StdErr, str::FromStr};

const SIMPLE_CODE_HASH: H256 =
    h256!("0xe1e354d6d643ad42724d40967e334984534e0367405c5ae42a9d7d63d77df419");

fn get_env_hex(name: &str) -> H256 {
    let value = std::env::var(name).expect("env var");
    // strip prefix 0x
    eprintln!("{}: {}", name, value);
    let value = value.trim_start_matches("0x");
    H256::from_str(&value).expect("parse hex")
}

fn gen_dev_udt_handler() -> SudtHandler {
    let simple_udt_tx = get_env_hex("NEXT_PUBLIC_SIMPLE_UDT_TX_HASH");

    let (out_point, script_id) = (
        OutPoint::new_builder()
            .tx_hash(simple_udt_tx.pack())
            .index(0u32.pack())
            .build(),
        ScriptId::new_data1(SIMPLE_CODE_HASH.clone()),
    );

    let cell_dep = CellDep::new_builder()
        .out_point(out_point)
        .dep_type(DepType::Code.into())
        .build();

    let udt_handler = ckb_sdk::transaction::handler::sudt::SudtHandler::new_with_customize(
        vec![cell_dep],
        script_id,
    );
    return udt_handler;
}

fn gen_dev_sighash_handler() -> Secp256k1Blake160SighashAllScriptHandler {
    let sighash_tx = get_env_hex("NEXT_PUBLIC_CKB_GENESIS_TX_1");

    let out_point = OutPoint::new_builder()
        .tx_hash(sighash_tx.pack())
        .index(0u32.pack())
        .build();

    let cell_dep = CellDep::new_builder()
        .out_point(out_point)
        .dep_type(DepType::DepGroup.into())
        .build();

    let sighash_handler =
        Secp256k1Blake160SighashAllScriptHandler::new_with_customize(vec![cell_dep]);
    return sighash_handler;
}

fn generate_configuration(
) -> Result<(NetworkInfo, TransactionBuilderConfiguration), Box<dyn StdErr>> {
    let network_info = NetworkInfo::devnet();
    let mut configuration =
        TransactionBuilderConfiguration::new_devnet().expect("new devnet configuration");

    configuration.register_script_handler(Box::new(gen_dev_sighash_handler()));
    configuration.register_script_handler(Box::new(gen_dev_udt_handler()));
    return Ok((network_info, configuration));
}

fn gen_udt_type_script(onwer_script: &Script) -> Script {
    let script = Script::new_builder()
        .code_hash(SIMPLE_CODE_HASH.pack())
        .hash_type(ScriptHashType::Data1.into())
        .args(onwer_script.calc_script_hash().as_bytes().pack())
        .build();
    return script;
}

fn init_or_send_udt(
    issuer_address: &str,
    sender_info: &(String, H256),
    receiver_address: Option<&str>,
    sudt_amount: u128,
    apply: bool,
) -> Result<(), Box<dyn StdErr>> {
    let (network_info, configuration) = generate_configuration()?;

    let issuer = Address::from_str(issuer_address)?;
    let sender = Address::from_str(&sender_info.0)?;
    let receiver = if let Some(addr) = receiver_address {
        Address::from_str(addr)?
    } else {
        sender.clone()
    };

    let iterator = InputIterator::new_with_address(&[sender.clone()], &network_info);
    let owner_mode = receiver_address.is_none();
    let mut builder = SudtTransactionBuilder::new(configuration, iterator, &issuer, owner_mode)?;
    builder.set_sudt_type_script(gen_udt_type_script(&(&issuer).into()));
    builder.add_output(&receiver, sudt_amount);

    let mut tx_with_groups = builder.build(&Default::default())?;

    let private_keys = vec![sender_info.1.clone()];

    TransactionSigner::new(&network_info).sign_transaction(
        &mut tx_with_groups,
        &SignContexts::new_sighash_h256(private_keys)?,
    )?;

    let json_tx = ckb_jsonrpc_types::TransactionView::from(tx_with_groups.get_tx_view().clone());
    println!("tx: {}", serde_json::to_string_pretty(&json_tx).unwrap());

    if apply {
        let tx_hash = CkbRpcClient::new(network_info.url.as_str())
            .send_transaction(json_tx.inner, None)
            .expect("send transaction");
        println!(">>> tx {} sent! <<<", tx_hash);
    } else {
        let result = CkbRpcClient::new(network_info.url.as_str())
            .test_tx_pool_accept(json_tx.inner.clone(), None)
            .expect("accept transaction");
        println!(">>> check tx result: {:?}  <<<", result);
    }

    Ok(())
}

fn generate_blocks(num: u64) -> Result<(), Box<dyn StdErr>> {
    let network_info = NetworkInfo::devnet();
    let rpc_client = CkbRpcClient::new(network_info.url.as_str());
    for i in 0..num {
        rpc_client.generate_block()?;
        // sleep 200ms
        std::thread::sleep(std::time::Duration::from_millis(200));
        eprintln!("block generated: {}", i);
    }
    Ok(())
}

fn generate_udt_type_script(address: &str) -> ckb_types::packed::Script {
    let address = Address::from_str(address).expect("parse address");
    let sudt_owner_lock_script: Script = (&address).into();

    let res = Script::new_builder()
        .code_hash(SIMPLE_CODE_HASH.pack())
        .hash_type(ScriptHashType::Data1.into())
        .args(sudt_owner_lock_script.calc_script_hash().as_bytes().pack())
        .build();
    res
}

fn get_nodes_info(node: &str) -> (String, H256) {
    let nodes_dir = std::env::var("NODES_DIR").expect("env var");
    let node_dir = format!("{}/{}", nodes_dir, node);
    let wallet =
        std::fs::read_to_string(format!("{}/ckb-chain/wallet", node_dir)).expect("read failed");
    let key = std::fs::read_to_string(format!("{}/ckb-chain/key", node_dir)).expect("read failed");
    eprintln!("node {}: wallet: {}", node, wallet);
    (wallet, H256::from_str(&key.trim()).expect("parse hex"))
}

fn main() -> Result<(), Box<dyn StdErr>> {
    let udt_owner = get_nodes_info("deployer");
    let wallets = [
        get_nodes_info("1"),
        get_nodes_info("2"),
        get_nodes_info("3"),
    ];

    init_or_send_udt(&udt_owner.0, &udt_owner, None, 1000000000000, true).expect("init udt");
    generate_blocks(4).expect("ok");

    init_or_send_udt(
        &udt_owner.0,
        &udt_owner,
        Some(&wallets[0].0),
        200000000000,
        true,
    )?;
    generate_blocks(4).expect("ok");

    init_or_send_udt(
        &udt_owner.0,
        &udt_owner,
        Some(&wallets[1].0),
        200000000000,
        true,
    )?;
    generate_blocks(4).expect("ok");

    init_or_send_udt(
        &udt_owner.0,
        &udt_owner,
        Some(&wallets[2].0),
        200000000000,
        true,
    )?;
    generate_blocks(4).expect("ok");

    let script = generate_udt_type_script(&udt_owner.0);
    println!("initialized udt_type_script: {} ...", script);

    Ok(())
}
