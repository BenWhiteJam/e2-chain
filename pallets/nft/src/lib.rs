#![cfg_attr(not(feature = "std"), no_std)]

use codec::FullCodec;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch,
    traits::{EnsureOrigin, Get},
    Hashable,
};
use frame_system::{self as system};
use sp_runtime::traits::{MaybeSerialize, Member};
use sp_std::{fmt::Debug, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait<I = DefaultInstance>: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type AssetAdmin: EnsureOrigin<Self::Origin>;
    type AssetInfo: Hashable + Member + MaybeSerialize + Debug + Default + FullCodec;
    type UserAssetLimit: Get<usize>;
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as NFT {
        // Mapping from holder address to their (enumerable) set of owned assets
        AssetsForAccount get(fn assets_for_account): map hasher(blake2_128_concat) T::AccountId => Vec<Vec<u8>>;
        // Mapping from asset ID to the address that owns it
        AccountForAsset get(fn account_for_asset): map hasher(identity) Vec<u8> => T::AccountId;
        // Mapping from asset ID to the info for that asset
        InfoForAsset get(fn info_for_asset): map hasher(identity) Vec<u8> => T::AssetInfo;
    }
}

decl_event!(
    pub enum Event<T, I = DefaultInstance>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        AssetMinted(Vec<u8>, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait<I>, I: Instance> {
        // The asset already exists
        AssetExists,
        // The user has too many assets
        TooManyAssetsForUser,
    }
}

// The pallet's dispatchable functions.
decl_module! {
    pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
        type Error = Error<T, I>;
        fn deposit_event() = default;

        #[weight = 10_000]
        pub fn mint_asset(origin, owner_account: T::AccountId, asset_info: T::AssetInfo) -> dispatch::DispatchResult {
            T::AssetAdmin::ensure_origin(origin)?;

            let asset_id = asset_info.blake2_128_concat();
            if InfoForAsset::<T, I>::contains_key(&asset_id) {
                Err(Error::<T, I>::AssetExists)?;
            }

            if AssetsForAccount::<T, I>::decode_len(&owner_account).unwrap_or(0) == T::UserAssetLimit::get() {
                Err(Error::<T, I>::TooManyAssetsForUser)?;
            }

            AssetsForAccount::<T, I>::append(&owner_account, &asset_id);
            AccountForAsset::<T, I>::insert(&asset_id, &owner_account);
            InfoForAsset::<T, I>::insert(&asset_id, asset_info);
            Self::deposit_event(RawEvent::AssetMinted(asset_id, owner_account));
            Ok(())
        }
    }
}
