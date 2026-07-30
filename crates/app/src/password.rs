use argon2::password_hash::{
    Error as PasswordHashError, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    rand_core::OsRng,
};
use argon2::{Argon2, Params};
use thiserror::Error;

const MAX_PASSWORD_BYTES: usize = 1024;
const MAX_HASH_BYTES: usize = 512;
const MAX_MEMORY_COST_KIB: u32 = 256 * 1024;
const MAX_TIME_COST: u32 = 10;
const MAX_PARALLELISM: u32 = 8;
const EXPECTED_SALT_B64_LENGTH: usize = 22;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("пароль не может быть пустым")]
    Empty,
    #[error("пароль превышает предел {MAX_PASSWORD_BYTES} байт")]
    TooLong,
    #[error("хеш пароля не соответствует безопасной политике")]
    UnsafeHash,
    #[error("операция Argon2id завершилась ошибкой")]
    Hash(#[from] PasswordHashError),
}

#[derive(Default)]
pub struct PasswordService {
    argon2: Argon2<'static>,
}

impl PasswordService {
    pub fn hash_password(&self, password: &str) -> Result<String, PasswordError> {
        validate_password(password)?;
        let salt = SaltString::generate(&mut OsRng);
        let hash = self.argon2.hash_password(password.as_bytes(), &salt)?;
        Ok(hash.to_string())
    }

    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, PasswordError> {
        validate_password(password)?;
        let parsed = parse_bounded_hash(hash)?;

        match self.argon2.verify_password(password.as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(PasswordHashError::Password) => Ok(false),
            Err(error) => Err(PasswordError::Hash(error)),
        }
    }

    pub fn needs_rehash(&self, hash: &str) -> Result<bool, PasswordError> {
        let parsed = parse_bounded_hash(hash)?;
        let defaults = Params::default();

        Ok(parsed.algorithm.as_str() != "argon2id"
            || parsed.version != Some(19)
            || parsed.params.get_decimal("m") != Some(defaults.m_cost())
            || parsed.params.get_decimal("t") != Some(defaults.t_cost())
            || parsed.params.get_decimal("p") != Some(defaults.p_cost())
            || parsed
                .salt
                .is_none_or(|salt| salt.len() != EXPECTED_SALT_B64_LENGTH)
            || parsed
                .hash
                .is_none_or(|hash| hash.len() != Params::DEFAULT_OUTPUT_LEN))
    }
}

fn validate_password(password: &str) -> Result<(), PasswordError> {
    if password.is_empty() {
        return Err(PasswordError::Empty);
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(PasswordError::TooLong);
    }
    Ok(())
}

fn parse_bounded_hash(hash: &str) -> Result<PasswordHash<'_>, PasswordError> {
    if hash.len() > MAX_HASH_BYTES {
        return Err(PasswordError::UnsafeHash);
    }

    let parsed = PasswordHash::new(hash)?;
    let memory = parsed.params.get_decimal("m");
    let time = parsed.params.get_decimal("t");
    let parallelism = parsed.params.get_decimal("p");

    if parsed.algorithm.as_str() != "argon2id"
        || memory.is_none_or(|value| value > MAX_MEMORY_COST_KIB)
        || time.is_none_or(|value| value > MAX_TIME_COST)
        || parallelism.is_none_or(|value| value > MAX_PARALLELISM)
    {
        return Err(PasswordError::UnsafeHash);
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use argon2::{Algorithm, Version};

    use super::*;

    #[test]
    fn hash_uses_argon2id() {
        let service = PasswordService::default();
        let hash = service.hash_password("password").expect("hash succeeds");

        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn hash_and_verify_password() {
        let service = PasswordService::default();
        let hash = service
            .hash_password("correct horse battery staple")
            .expect("hash succeeds");

        assert!(
            service
                .verify_password("correct horse battery staple", &hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn wrong_password_is_rejected() {
        let service = PasswordService::default();
        let hash = service
            .hash_password("correct password")
            .expect("hash succeeds");

        assert!(
            !service
                .verify_password("wrong password", &hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn hashes_are_unique_for_same_password() {
        let service = PasswordService::default();
        let first = service
            .hash_password("same password")
            .expect("hash succeeds");
        let second = service
            .hash_password("same password")
            .expect("hash succeeds");

        assert_ne!(first, second);
    }

    #[test]
    fn rejects_excessive_hash_cost_before_verification() {
        let service = PasswordService::default();
        let hostile = "$argon2id$v=19$m=4294967295,t=2,p=1$c2FsdHNhbHQ$\
                       MDEyMzQ1Njc4OWFiY2RlZg";

        assert!(matches!(
            service.verify_password("password", hostile),
            Err(PasswordError::UnsafeHash)
        ));
    }

    #[test]
    fn fresh_hash_does_not_need_rehash() {
        let service = PasswordService::default();
        let hash = service.hash_password("password").expect("hash succeeds");

        assert!(!service.needs_rehash(&hash).expect("hash is valid"));
    }

    #[test]
    fn short_digest_needs_rehash() {
        let defaults = Params::default();
        let short_params = Params::new(
            defaults.m_cost(),
            defaults.t_cost(),
            defaults.p_cost(),
            Some(16),
        )
        .expect("short test output remains valid Argon2");
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, short_params);
        let salt = SaltString::generate(&mut OsRng);
        let hash = argon2
            .hash_password(b"password", &salt)
            .expect("fixture hash succeeds")
            .to_string();

        assert!(
            PasswordService::default()
                .needs_rehash(&hash)
                .expect("fixture hash is valid")
        );
    }
}
