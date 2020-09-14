#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::codec::{Decode, Encode};
use frame_support::traits::{Currency, ExistenceRequirement::AllowDeath, Time, Vec};
use frame_support::{decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure};
use frame_system::{self, ensure_signed};
use secp256k1;

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: frame_system::Trait {
    /// Because this pallet emits events, it depends on the runtime's definition of an event.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type Currency: Currency<Self::AccountId>;
    type Timestamp: Time;
}

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

type Moment<T> = <<T as Trait>::Timestamp as Time>::Moment;

type ChannelOf<T> = Chan<<T as frame_system::Trait>::AccountId, Moment<T>>;

// struct to store the registered Device Informatin
#[derive(Decode, Encode, Default)]
pub struct Chan<AccountId, Timestamp> {
    sender: AccountId,
    receiver: AccountId,
    expiration: Timestamp,
}

// events
decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        Timestamp = Moment<T>,
        Balance = BalanceOf<T>,
    {
        ChannelOpened(AccountId, AccountId, Timestamp),
        ChannelClosed(AccountId, AccountId, Timestamp),
        ClaimPayment(AccountId, AccountId, Balance),
    }
);

// storage for this module
decl_storage! {
  trait Store for Module<T: Trait> as Device {
      Channel get(fn get_channel): map hasher(blake2_128_concat) (T::AccountId, T::AccountId) => ChannelOf<T>;
      Nonce get(fn get_nonce): double_map hasher(blake2_128_concat) (T::AccountId, T::AccountId), hasher(blake2_128_concat) u32 => bool;
  }

}

// public interface for this runtime module
decl_module! {
  pub struct Module<T: Trait> for enum Call where origin: T::Origin {
      // initialize the default event for this module
      fn deposit_event() = default;

      #[weight = 10_000]
      pub fn open_channel(origin, receiver: T::AccountId) -> DispatchResult {
          let sender = ensure_signed(origin)?;
          ensure!(!Channel::<T>::contains_key((sender.clone(),receiver.clone())), "Channel already opened");
          ensure!(sender.clone() != receiver.clone(), "Channel should connect two different accounts");
          let time = T::Timestamp::now();
          let chan = ChannelOf::<T>{
              sender: sender.clone(),
              receiver: receiver.clone(),
              expiration: time.clone(),
          };
          Channel::<T>::insert((sender.clone(),receiver.clone()), chan);
          Self::deposit_event(RawEvent::ChannelOpened(sender,receiver, time));
          Ok(())
      }

      #[weight = 10_000]
      // make sure claim your payment before close the channel, TODO: add safty guard
      pub fn close_channel(origin, sender: T::AccountId) -> DispatchResult {
          // only receiver can close the channel
          let receiver = ensure_signed(origin)?;
          ensure!(Channel::<T>::contains_key((sender.clone(),receiver.clone())), "Channel not exists");

          // remove all the nonce of given channel
          Nonce::<T>::remove_prefix((sender.clone(),receiver.clone()));
          Channel::<T>::remove((sender.clone(),receiver.clone()));
          let time = T::Timestamp::now();
          Self::deposit_event(RawEvent::ChannelClosed(sender,receiver, time));

          Ok(())
      }

      #[weight = 10_000]
      pub fn claim_payment(origin, sender: T::AccountId, nonce: u32, amount: BalanceOf<T>, signature: Vec<u8>) -> DispatchResult {
          let receiver = ensure_signed(origin)?;
          ensure!(Channel::<T>::contains_key((sender.clone(),receiver.clone())), "Channel not exists");

          ensure!(!Nonce::<T>::contains_key((sender.clone(),receiver.clone()),nonce), "Nonce already consumed");
          Self::verify_signature(&sender, &receiver, nonce, amount, &signature)?;

          T::Currency::transfer(&sender, &receiver, amount, AllowDeath)?; // TODO: check what is AllowDeath
          Nonce::<T>::insert((sender.clone(),receiver.clone()), nonce, true); // mark nonce as used
          Self::deposit_event(RawEvent::ClaimPayment(sender, receiver, amount));
          Ok(())
      }
  }
}

impl<T: Trait> Module<T> {
    // TODO: verify signature, signature is on hash of |receiver_addr|nonce|amount|
    // nonce represents session_id, during one session, a sender can send multiple accumulated
    // micropayments with the same nonce; the receiver can only claim one payment of the same
    // nonce, i.e. the latest accumulated micropayment.
    pub fn verify_signature(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        nonce: u32,
        amount: BalanceOf<T>,
        signature: &Vec<u8>,
    ) -> DispatchResult {
        let mut pk = [0u8; 33];
        pk.copy_from_slice(&sender.encode());
        let pub_key = secp256k1::PublicKey::parse_compressed(&pk);
        ensure!(pub_key.is_ok(), "Invalid Pubkey");

        let signature = secp256k1::Signature::parse_slice(signature);
        ensure!(signature.is_ok(), "Invalid Signature");

        let hash = Self::construct_byte_array_and_hash(&receiver, nonce, amount);
        let message = secp256k1::Message::parse(&hash);

        let verified = secp256k1::verify(&message, &signature.unwrap(), &pub_key.unwrap());
        ensure!(verified, "Fail to verify");

        Ok(())
    }

    // construct data from |receiver_addr|nonce|amount| and hash it
    fn construct_byte_array_and_hash(
        address: &T::AccountId,
        nonce: u32,
        amount: BalanceOf<T>,
    ) -> [u8; 32] {
        let mut data = Vec::new();
        data.extend_from_slice(&address.encode());
        data.extend_from_slice(&nonce.to_be_bytes());
        data.extend_from_slice(&amount.encode());
        let hash = sp_io::hashing::blake2_256(&data);
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake2_hash() {
        let alice: [u8; 32] = [
            212, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189, 4, 169, 159, 214, 130, 44, 133,
            88, 133, 76, 205, 227, 154, 86, 132, 231, 165, 109, 162, 125,
        ];
        let nonce: u32 = 22;
        let amount: u128 = 100;
        let mut data = Vec::new();

        let should_be: [u8; 32] = [
            162, 225, 249, 9, 223, 71, 169, 240, 180, 154, 247, 135, 145, 15, 230, 200, 24, 9, 21,
            249, 253, 78, 123, 105, 135, 191, 146, 220, 204, 18, 247, 124,
        ];

        data.extend_from_slice(&alice);
        data.extend_from_slice(&nonce.to_be_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        let hash = sp_io::hashing::blake2_256(&data);
        assert_eq!(&hash, &should_be);
    }
}
