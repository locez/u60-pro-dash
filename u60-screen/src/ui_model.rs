//! Keep repeater instances alive; unchanged rows emit no model notification.
use slint::{Model, ModelRc, VecModel};

pub fn reconcile<T: Clone + PartialEq + 'static, K: PartialEq>(
    current: ModelRc<T>,
    next: Vec<T>,
    key: impl Fn(&T) -> K,
) -> ModelRc<T> {
    let Some(model) = current.as_any().downcast_ref::<VecModel<T>>() else {
        return std::rc::Rc::new(VecModel::from(next)).into();
    };
    for (index, value) in next.iter().enumerate() {
        if let Some(old) = model.row_data(index) {
            if key(&old) == key(value) {
                if old != *value {
                    model.set_row_data(index, value.clone());
                }
                continue;
            }
            if let Some(found) = ((index + 1)..model.row_count())
                .find(|&i| model.row_data(i).is_some_and(|row| key(&row) == key(value)))
            {
                model.remove(found);
            }
        }
        model.insert(index, value.clone());
    }
    while model.row_count() > next.len() {
        model.remove(model.row_count() - 1);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_models_keep_identity_and_new_records_keep_existing_order() {
        let rows = vec![("a", 1), ("b", 2), ("c", 3)];
        let old: ModelRc<_> = std::rc::Rc::new(VecModel::from(rows.clone())).into();
        let updated = reconcile(old.clone(), rows, |row| row.0);
        assert!(old == updated);
        let next = vec![("new", 4), ("a", 1), ("b", 20), ("c", 3)];
        assert!(old == reconcile(updated, next.clone(), |row| row.0));
        assert_eq!(old.iter().collect::<Vec<_>>(), next);
        let next = vec![("c", 3), ("a", 1)];
        reconcile(old.clone(), next.clone(), |row| row.0);
        assert_eq!(old.iter().collect::<Vec<_>>(), next);
    }
}
