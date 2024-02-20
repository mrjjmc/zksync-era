// Define constants
const DEFAULT_GAS_LIMIT: u64 = 300_000;
const DEFAULT_PRIORITY_FEE: u64 = 2_000_000_000;
const ETH_DEPOSIT_GAS_LIMIT: u64 = 200_000;

impl<S: EthereumSigner> EthereumProvider<S> {
    // Other methods...

    /// Sends a transaction to ERC20 token contract to approve the ERC20 deposit.
    pub async fn approve_erc20_token_deposits(
        &self,
        token_address: Address,
        bridge: Option<Address>,
    ) -> Result<H256, ClientError> {
        self.limited_approve_erc20_token_deposits(token_address, U256::max_value(), bridge)
            .await
    }

    /// Sends a transaction to ERC20 token contract to approve the limited ERC20 deposit.
    pub async fn limited_approve_erc20_token_deposits(
        &self,
        token_address: Address,
        max_erc20_approve_amount: U256,
        bridge: Option<Address>,
    ) -> Result<H256, ClientError> {
        let bridge = bridge.unwrap_or(self.default_bridges.l1_erc20_default_bridge);
        let contract_function = self
            .erc20_abi
            .function("approve")
            .expect("failed to get function parameters");
        let params = (bridge, max_erc20_approve_amount);
        let data = contract_function
            .encode_input(&params.into_tokens())
            .expect("failed to encode parameters");

        let options = Options {
            gas: Some(U256::from(DEFAULT_GAS_LIMIT)),
            ..Default::default()
        };

        let signed_tx = self
            .client()
            .sign_prepared_tx_for_addr(data, token_address, options, "provider")
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transaction_hash = self
            .client()
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Performs a deposit in zkSync network.
    pub async fn deposit(
        &self,
        l1_token_address: Address,
        amount: U256,
        to: Address,
        operator_tip: Option<U256>,
        bridge_address: Option<Address>,
        eth_options: Option<Options>,
    ) -> Result<H256, ClientError> {
        let operator_tip = operator_tip.unwrap_or_default();
        let is_eth_deposit = l1_token_address == Address::zero();

        let gas_limit = if is_eth_deposit {
            ETH_DEPOSIT_GAS_LIMIT
        } else {
            // Use default gas limit for ERC20 deposits
            DEFAULT_GAS_LIMIT
        };

        let options = eth_options.unwrap_or_default();

        let gas_price = options.gas_price.unwrap_or_else(|| {
            self.client()
                .get_gas_price("zksync-rs")
                .await
                .unwrap_or_default()
        });

        // Base cost calculation
        let base_cost = self
            .base_cost(
                U256::from(DEFAULT_GAS_LIMIT),
                L1_TO_L2_GAS_PER_PUBDATA,
                Some(gas_price),
            )
            .await?;

        let total_value = if is_eth_deposit {
            base_cost + operator_tip + amount
        } else {
            base_cost + operator_tip
        };

        let options = Options {
            gas: Some(U256::from(gas_limit)),
            value: Some(total_value),
            ..options
        };

        let transaction_hash = if is_eth_deposit {
            self.request_execute(to, amount, Default::default(), U256::from(3_000_000u32), None, None, Some(gas_price), Default::default()).await?
        } else {
            let bridge_address = bridge_address.unwrap_or(self.default_bridges.l1_erc20_default_bridge);
            let contract_function = self.l1_bridge_abi.function("deposit").expect("failed to get function parameters");
            let params = (to, l1_token_address, amount, U256::from(3_000_000u32), U256::from(L1_TO_L2_GAS_PER_PUBDATA));
            let data = contract_function.encode_input(&params.into_tokens()).expect("failed to encode parameters");

            let signed_tx = self
                .client()
                .sign_prepared_tx_for_addr(data, bridge_address, options, "provider")
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?;

            self.client()
                .send_raw_tx(signed_tx.raw_tx)
                .await
                .map_err(|err| ClientError::NetworkError(err.to_string()))?
        };

        Ok(transaction_hash)
    }

    // Other methods...
}
