/**
 * Form validation utilities for data entry.
 */

/** Character ID format: lowercase letters, digits, underscores */
const CHAR_ID_RE = /^[a-z0-9_]+$/;

/** Enemy ID format: typically lowercase letters, digits, underscores */
const ENEMY_ID_RE = /^[a-z0-9_]+$/;

/**
 * Validate a character ID string.
 */
export function validateCharId(value) {
  if (!value) return "Character ID is required";
  if (!CHAR_ID_RE.test(value)) return "Character ID must be lowercase letters, digits, or underscores";
  return null;
}

/**
 * Validate an enemy ID string.
 */
export function validateEnemyId(value) {
  if (!value) return "Enemy ID is required";
  if (!ENEMY_ID_RE.test(value)) return "Enemy ID must be lowercase letters, digits, or underscores";
  return null;
}

/**
 * Validate a numeric level (1-60).
 */
export function validateLevel(value) {
  const num = Number(value);
  if (isNaN(num) || !Number.isInteger(num)) return "Level must be an integer";
  if (num < 1 || num > 60) return "Level must be between 1 and 60";
  return null;
}

/**
 * Validate ascension (0-6).
 */
export function validateAscension(value) {
  const num = Number(value);
  if (isNaN(num) || !Number.isInteger(num)) return "Ascension must be an integer";
  if (num < 0 || num > 6) return "Ascension must be between 0 and 6";
  return null;
}

/**
 * Validate a critical rate/damage value (0.0-1.0).
 */
export function validateCritRate(value) {
  const num = Number(value);
  if (isNaN(num)) return "Crit value must be a number";
  if (num < 0 || num > 1) return "Crit value must be between 0 and 1";
  return null;
}

/**
 * Validate a non-negative number.
 */
export function validateNonNegative(value, label = "Value") {
  const num = Number(value);
  if (isNaN(num)) return `${label} must be a number`;
  if (num < 0) return `${label} must be non-negative`;
  return null;
}

/**
 * Validate a required field.
 */
export function validateRequired(value, label = "Field") {
  if (value === null || value === undefined || value === "") return `${label} is required`;
  return null;
}

/**
 * Validate drive disc slot (1-6).
 */
export function validateSlot(value) {
  const num = Number(value);
  if (isNaN(num) || !Number.isInteger(num)) return "Slot must be an integer";
  if (num < 1 || num > 6) return "Slot must be between 1 and 6";
  return null;
}

/**
 * Validate an equipment ID.
 */
export function validateEquipmentId(value) {
  if (!value) return "ID is required";
  return null;
}

/**
 * Validate a set ID (disc set).
 */
export function validateSetId(value) {
  if (!value) return "Set ID is required";
  return null;
}

/**
 * Run multiple validators against a value.
 * Returns the first error message, or null if all pass.
 *
 * @param {*} value
 * @param {Array<function>} validators
 * @returns {string|null}
 */
export function validate(value, validators) {
  for (const fn of validators) {
    const err = fn(value);
    if (err) return err;
  }
  return null;
}

/**
 * Validate an entire form field map.
 * Returns an object of { fieldKey: errorMessage } for invalid fields.
 *
 * @param {Record<string, { value: *, validators: Array<function> }>} fieldMap
 * @returns {Record<string, string>}
 */
export function validateForm(fieldMap) {
  const errors = {};
  for (const [key, { value, validators }] of Object.entries(fieldMap)) {
    const err = validate(value, validators);
    if (err) errors[key] = err;
  }
  return errors;
}
