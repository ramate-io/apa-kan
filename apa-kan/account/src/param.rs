//! Single place for the account signing parameter set (Decision #58-Q20: **MlDsa65**).
//! Workspace `ml-dsa` pin: OPEN-Q #58-19 (`=0.1.0-rc.7` until joint bump with gwrdfa).

use ml_dsa::MlDsa65;

/// ML-DSA parameter set for Apa Kan accounts. Intentionally **not** gwrdfa's MlDsa44.
pub type AccountMlDsaParams = MlDsa65;
