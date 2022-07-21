#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Empty};
    use cw721_base::{ExecuteMsg, InstantiateMsg, MintMsg};
    use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};

    /// Name of the NFT collection
    const NAME: &str = "Interchain Nifties";
    /// Symbol ticker for the NFT collection
    const SYMBOL: &str = "ICSNFT";
    /// In reality the minter will be the ICS contract
    const MINTER: &str = "stars1minter";

    /// Other addresses representing normal users
    const ADDR1: &str = "stars1yyy";
    const ADDR2: &str = "stars1xxx";
    const ADDR3: &str = "stars1zzz";

    fn contract_cw721_ics() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );
        Box::new(contract)
    }

    fn mock_app() -> App {
        App::default()
    }

    fn instantiate_cw721_ics(app: &mut App) -> Addr {
        let code_id = app.store_code(contract_cw721_ics());
        let msg = InstantiateMsg {
            name: NAME.to_string(),
            symbol: SYMBOL.to_string(),
            minter: MINTER.to_string(),
        };

        app.instantiate_contract(
            code_id,
            Addr::unchecked(MINTER),
            &msg,
            &[],
            "cw721-ics",
            None,
        )
        .unwrap()
    }

    fn mint(
        app: &mut App,
        addr: Addr,
        sender: &str,
        token_id: &str,
        owner: &str,
        token_uri: Option<String>,
    ) -> anyhow::Result<AppResponse> {
        // TODO: mint msg may change
        let mint_msg = MintMsg::<Empty> {
            token_id: token_id.to_string(),
            owner: owner.to_string(),
            token_uri,
            extension: Empty {},
        };
        let msg = ExecuteMsg::Mint(mint_msg);

        app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
    }

    fn transfer_nft(
        app: &mut App,
        addr: Addr,
        sender: &str,
        recipient: &str,
        token_id: &str,
    ) -> anyhow::Result<AppResponse> {
        let msg = ExecuteMsg::<Empty>::TransferNft {
            recipient: recipient.to_string(),
            token_id: token_id.to_string(),
        };
        app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
    }

    fn add_approval(
        app: &mut App,
        addr: Addr,
        sender: &str,
        spender: &str,
        token_id: &str,
    ) -> anyhow::Result<AppResponse> {
        let msg = ExecuteMsg::<Empty>::Approve {
            spender: spender.to_string(),
            token_id: token_id.to_string(),
            expires: None,
        };
        app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
    }

    fn add_operator(
        app: &mut App,
        addr: Addr,
        sender: &str,
        operator: &str,
    ) -> anyhow::Result<AppResponse> {
        let msg = ExecuteMsg::<Empty>::ApproveAll {
            operator: operator.to_string(),
            expires: None,
        };
        app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
    }

    /// No logic has changed just a sanity check I set this up correct
    #[test]
    fn test_instantiate() {
        let mut app = mock_app();
        let _addr = instantiate_cw721_ics(&mut app);
    }

    #[test]
    fn test_transfer_nft_valid() {
        let mut app = mock_app();
        let addr = instantiate_cw721_ics(&mut app);

        // Mint token_id 1 to ADDR1
        let token_id = "1";
        mint(&mut app, addr.clone(), MINTER, token_id, ADDR1, None).unwrap();

        // ADDR1 transfers to ADDR2, valid as ADDR1 owns the token
        transfer_nft(&mut app, addr.clone(), ADDR1, ADDR2, token_id).unwrap();

        // Now ADDR2 owns the token, let's approve ADDR3 and let them transfer it back to ADDR1
        add_approval(&mut app, addr.clone(), ADDR2, ADDR3, token_id).unwrap();

        // Now try and transfer back to ADDR1 as the approved ADDR3
        transfer_nft(&mut app, addr.clone(), ADDR3, ADDR1, token_id).unwrap();

        // ADDR1 now owns and approvals have been reset, let's add ADDR3 as an operator and try
        // to transfer back to ADDR2
        add_operator(&mut app, addr.clone(), ADDR1, ADDR3).unwrap();

        // Now try to transfer back to ADDR2 as the operator ADDR3
        transfer_nft(&mut app, addr.clone(), ADDR3, ADDR2, token_id).unwrap();

        // Now try to transfer back to ADDR1 as the minter MINTER
        transfer_nft(&mut app, addr, MINTER, ADDR1, token_id).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_transfer_nft_invalid() {
        let mut app = mock_app();
        let addr = instantiate_cw721_ics(&mut app);

        // Mint token_id 1 to ADDR1
        let token_id = "1";
        mint(&mut app, addr.clone(), MINTER, token_id, ADDR1, None).unwrap();

        // Try and transfer as non-owner, non-approved, non-operator, non-minter
        transfer_nft(&mut app, addr, ADDR2, ADDR3, token_id).unwrap();
    }
}
