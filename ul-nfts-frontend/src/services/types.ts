import { Uint128 } from "@cosmjs/cosmwasm-stargate";

export interface LiquidityPosition {
  pool_id: string;
  token_pair: [string, string];
  amount: Uint128;
  chain_id: string;
}

export interface NFTMetadata {
  tokenId: string;
  positions: LiquidityPosition[];
  owner: string;
  tokenURI?: string;
}

export interface MarketplaceListing {
  listingId: string;
  tokenId: string;
  seller: string;
  price: Uint128;
  createdAt: number;
}