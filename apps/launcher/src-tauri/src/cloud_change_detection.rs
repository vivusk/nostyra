//! Which units need a sync: changed in the cloud (`index.json` generation ≠ this
//! account's base generation, or a repair flag) or changed on this PC (stat-only
//! snapshot vs the base; only files whose size / mtime moved are hashed).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cloud_sync::{
    changed_units, read_index, scope_maps, snapshot_local_dir, BaseManifest, BaseManifestStore,
    CloudError, SyncScope, SyncUnit,
};

use crate::cloud_provider_factory::CloudAccount;

/// Keys of units changed in the cloud (index generation ≠ base generation, repair
/// flags) or on this PC (in-scope files differ from the base).
pub fn changed_or_dirty(
    account: &CloudAccount,
    data_root: &Path,
    scope: SyncScope,
    units: &BTreeMap<String, (SyncUnit, Vec<i64>)>,
) -> Result<BTreeSet<String>, CloudError> {
    let (index, _) = read_index(account.provider.as_ref())?;
    let bases = BaseManifestStore::new(data_root, &account.account_key);
    let mut generations = BTreeMap::new();
    let mut keys = BTreeSet::new();
    for (key, (unit, _)) in units {
        let base = bases.load(unit).ok().flatten();
        if let Some(base) = &base {
            generations.insert(key.clone(), base.manifest.generation);
        }
        if local_dirty(unit, base.as_ref(), &scope) {
            keys.insert(key.clone());
        }
    }
    keys.extend(
        changed_units(&index, &generations)
            .into_iter()
            .filter(|key| units.contains_key(key)),
    );
    Ok(keys)
}

/// Whether the unit's in-scope local files differ from its base (no base: any
/// in-scope file). Unreadable folder = dirty (the sync reports the error).
pub fn local_dirty(unit: &SyncUnit, base: Option<&BaseManifest>, scope: &SyncScope) -> bool {
    let Ok(snapshot) = snapshot_local_dir(unit.local_dir(), base, scope) else {
        return true;
    };
    let maps = scope_maps(&snapshot, base, None, scope);
    match base {
        Some(_) => maps.local != maps.base,
        None => !maps.local.is_empty(),
    }
}
