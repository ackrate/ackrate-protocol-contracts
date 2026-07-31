use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    UnsupportedVersion = 1,
    WrongNetwork = 2,
    InvalidAmount = 3,
    InvalidWindow = 4,
    PolicyAlreadyInstalled = 5,
    PolicyNotFound = 6,
    MandateMismatch = 7,
    MandateInactive = 8,
    PolicyTooLong = 9,
    PaymentCapExceeded = 10,
    PolicyHashMismatch = 11,
    Replay = 12,
    ReentrantExecution = 13,
    RequestTooLong = 14,
}
