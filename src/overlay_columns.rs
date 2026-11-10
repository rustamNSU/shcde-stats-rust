use crate::config::ColumnVisibility;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnId {
    Population,
    FoodEffect,
    TaxEffect,
    FearFactor,
    Popularity,
    Gold,
    AcquiredGold,
    HorseArchers,
    Army,
    CamelLancers,
    Shooters,
    Knights,
    ProducedFood,
    ProducedWeapons,
    ProducedStone,
    ProducedIron,
    ProducedWood,
    ArmyKilled,
    ArmyLost,
}

pub const ALL: [ColumnId; 19] = ColumnId::ALL;
pub const COLUMN_GAP: f32 = 6.0;

impl ColumnId {
    pub const ALL: [Self; 19] = [
        Self::Population,
        Self::FoodEffect,
        Self::TaxEffect,
        Self::FearFactor,
        Self::Popularity,
        Self::Gold,
        Self::AcquiredGold,
        Self::HorseArchers,
        Self::Army,
        Self::CamelLancers,
        Self::Shooters,
        Self::Knights,
        Self::ProducedFood,
        Self::ProducedWeapons,
        Self::ProducedStone,
        Self::ProducedIron,
        Self::ProducedWood,
        Self::ArmyKilled,
        Self::ArmyLost,
    ];

    pub fn parse(key: &str) -> Option<Self> {
        ALL.into_iter().find(|column| column.key() == key)
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Population => "population",
            Self::FoodEffect => "food_effect",
            Self::TaxEffect => "tax_effect",
            Self::FearFactor => "fear_factor",
            Self::Popularity => "popularity",
            Self::Gold => "gold",
            Self::AcquiredGold => "acquired_gold",
            Self::HorseArchers => "horse_archers",
            Self::Army => "army",
            Self::CamelLancers => "camel_lancers",
            Self::Shooters => "shooters",
            Self::Knights => "knights",
            Self::ProducedFood => "produced_food",
            Self::ProducedWeapons => "produced_weapons",
            Self::ProducedStone => "produced_stone",
            Self::ProducedIron => "produced_iron",
            Self::ProducedWood => "produced_wood",
            Self::ArmyKilled => "army_killed",
            Self::ArmyLost => "army_lost",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Population => "Pop",
            Self::FoodEffect => "Food",
            Self::TaxEffect => "Tax",
            Self::FearFactor => "FF",
            Self::Popularity => "Popul.",
            Self::Gold => "Gold",
            Self::AcquiredGold => "Earned",
            Self::HorseArchers => "HA",
            Self::Army => "Army",
            Self::CamelLancers => "CL",
            Self::Shooters => "Ranged",
            Self::Knights => "Knight",
            Self::ProducedFood => "P.Food",
            Self::ProducedWeapons => "P.Arms",
            Self::ProducedStone => "P.Stone",
            Self::ProducedIron => "P.Iron",
            Self::ProducedWood => "P.Wood",
            Self::ArmyKilled => "Kills",
            Self::ArmyLost => "Losses",
        }
    }

    pub fn full_label(self) -> &'static str {
        match self {
            Self::Population => "Population",
            Self::FoodEffect => "Food ration effect",
            Self::TaxEffect => "Tax effect",
            Self::FearFactor => "Fear factor",
            Self::Popularity => "Popularity",
            Self::Gold => "Gold",
            Self::AcquiredGold => "Acquired gold",
            Self::HorseArchers => "Horse archers",
            Self::Army => "Army size",
            Self::CamelLancers => "Camel lancers",
            Self::Shooters => "Ranged units",
            Self::Knights => "Knights",
            Self::ProducedFood => "Produced food",
            Self::ProducedWeapons => "Produced weapons",
            Self::ProducedStone => "Produced stone",
            Self::ProducedIron => "Produced iron",
            Self::ProducedWood => "Produced wood",
            Self::ArmyKilled => "Army killed",
            Self::ArmyLost => "Army lost",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Self::Population
            | Self::FoodEffect
            | Self::TaxEffect
            | Self::FearFactor
            | Self::Popularity
            | Self::Gold
            | Self::AcquiredGold => "Economy",
            Self::ProducedFood
            | Self::ProducedWeapons
            | Self::ProducedStone
            | Self::ProducedIron
            | Self::ProducedWood => "Production",
            _ => "Army",
        }
    }

    pub fn width(self, font: f32) -> f32 {
        // Floors retain readable headers and separate the drag grip from the remove button.
        let (base, minimum): (f32, f32) = match self {
            Self::Population => (44.0, 32.0),
            Self::FoodEffect => (40.0, 34.0),
            Self::TaxEffect => (40.0, 30.0),
            Self::FearFactor => (30.0, 28.0),
            Self::Popularity => (48.0, 42.0),
            Self::Gold => (60.0, 36.0),
            Self::AcquiredGold => (70.0, 46.0),
            Self::HorseArchers | Self::CamelLancers => (44.0, 28.0),
            Self::Army => (44.0, 36.0),
            Self::Shooters => (52.0, 46.0),
            Self::Knights => (44.0, 44.0),
            Self::ProducedFood | Self::ProducedWeapons => (70.0, 44.0),
            Self::ProducedStone | Self::ProducedWood => (70.0, 48.0),
            Self::ProducedIron => (70.0, 40.0),
            Self::ArmyKilled => (50.0, 34.0),
            Self::ArmyLost => (50.0, 44.0),
        };
        minimum.max(base * font / 16.0)
    }

    pub fn enabled(self, columns: &ColumnVisibility) -> bool {
        match self {
            Self::Population => columns.population,
            Self::FoodEffect => columns.food_effect,
            Self::TaxEffect => columns.tax_effect,
            Self::FearFactor => columns.fear_factor,
            Self::Popularity => columns.total_effect,
            Self::Gold => columns.gold,
            Self::AcquiredGold => columns.acquired_gold,
            Self::HorseArchers => columns.ha,
            Self::Army => columns.army,
            Self::CamelLancers => columns.cl,
            Self::Shooters => columns.shooters,
            Self::Knights => columns.knights,
            Self::ProducedFood => columns.produced_food,
            Self::ProducedWeapons => columns.produced_weapons,
            Self::ProducedStone => columns.produced_stone,
            Self::ProducedIron => columns.produced_iron,
            Self::ProducedWood => columns.produced_wood,
            Self::ArmyKilled => columns.army_killed,
            Self::ArmyLost => columns.army_lost,
        }
    }

    pub fn set_enabled(self, columns: &mut ColumnVisibility, enabled: bool) {
        match self {
            Self::Population => columns.population = enabled,
            Self::FoodEffect => columns.food_effect = enabled,
            Self::TaxEffect => columns.tax_effect = enabled,
            Self::FearFactor => columns.fear_factor = enabled,
            Self::Popularity => columns.total_effect = enabled,
            Self::Gold => columns.gold = enabled,
            Self::AcquiredGold => columns.acquired_gold = enabled,
            Self::HorseArchers => columns.ha = enabled,
            Self::Army => columns.army = enabled,
            Self::CamelLancers => columns.cl = enabled,
            Self::Shooters => columns.shooters = enabled,
            Self::Knights => columns.knights = enabled,
            Self::ProducedFood => columns.produced_food = enabled,
            Self::ProducedWeapons => columns.produced_weapons = enabled,
            Self::ProducedStone => columns.produced_stone = enabled,
            Self::ProducedIron => columns.produced_iron = enabled,
            Self::ProducedWood => columns.produced_wood = enabled,
            Self::ArmyKilled => columns.army_killed = enabled,
            Self::ArmyLost => columns.army_lost = enabled,
        }
    }
}

pub fn default_order() -> Vec<String> {
    ALL.into_iter()
        .map(|column| column.key().to_owned())
        .collect()
}

pub fn normalize_order(order: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(ALL.len());
    for key in order {
        if ColumnId::parse(key).is_some() && !result.contains(key) {
            result.push(key.clone());
        }
    }
    for column in ALL {
        let key = column.key();
        if !result.iter().any(|entry| entry == key) {
            result.push(key.to_owned());
        }
    }
    result
}

pub fn visible_columns(order: &[String], columns: &ColumnVisibility) -> Vec<ColumnId> {
    normalize_order(order)
        .iter()
        .filter_map(|key| ColumnId::parse(key))
        .filter(|column| column.enabled(columns))
        .collect()
}

/// Moves a column before another column, retaining hidden columns in the order.
/// A missing destination means the end of the full saved order.
pub fn move_before(order: &mut Vec<String>, id: &str, before: Option<&str>) -> bool {
    move_column(order, id, before, false)
}

/// Positions an added column after the selected column, or at the end if none is selected.
pub fn place_after(order: &mut Vec<String>, id: &str, after: Option<&str>) -> bool {
    move_column(order, id, after, true)
}

fn move_column(order: &mut Vec<String>, id: &str, anchor: Option<&str>, after: bool) -> bool {
    if ColumnId::parse(id).is_none()
        || anchor == Some(id)
        || anchor.is_some_and(|key| ColumnId::parse(key).is_none())
    {
        return false;
    }

    let mut updated = normalize_order(order);
    updated.retain(|key| key != id);
    let destination = anchor
        .and_then(|key| updated.iter().position(|entry| entry == key))
        .map(|index| index + usize::from(after))
        .unwrap_or(updated.len());
    updated.insert(destination, id.to_owned());
    if *order == updated {
        return false;
    }
    *order = updated;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    #[test]
    fn normalization_preserves_custom_order_and_appends_missing_columns() {
        let input = keys(&["army", "unknown", "gold", "army", "produced_iron", ""]);
        let normalized = normalize_order(&input);
        assert_eq!(&normalized[..3], keys(&["army", "gold", "produced_iron"]));
        assert_eq!(normalized.len(), ALL.len());
        assert_eq!(normalize_order(&normalized), normalized);
        assert_eq!(normalize_order(&[]), default_order());
        for column in ALL {
            assert_eq!(ColumnId::parse(column.key()), Some(column));
            assert_eq!(
                normalized.iter().filter(|key| *key == column.key()).count(),
                1
            );
        }
    }

    #[test]
    fn moving_visible_columns_keeps_hidden_columns_in_the_saved_order() {
        let mut order = default_order();
        let mut visibility = ColumnVisibility::default();
        for column in ALL {
            column.set_enabled(&mut visibility, false);
        }
        visibility.population = true;
        visibility.gold = true;
        visibility.army = true;

        assert!(move_before(&mut order, "army", Some("gold")));
        assert_eq!(
            visible_columns(&order, &visibility),
            vec![ColumnId::Population, ColumnId::Army, ColumnId::Gold]
        );
        assert_eq!(order.len(), ALL.len());
        assert_eq!(&order[..5], &default_order()[..5]);
        ColumnId::FearFactor.set_enabled(&mut visibility, true);
        assert_eq!(
            visible_columns(&order, &visibility),
            vec![
                ColumnId::Population,
                ColumnId::FearFactor,
                ColumnId::Army,
                ColumnId::Gold
            ]
        );
    }

    #[test]
    fn adding_after_selection_and_moving_to_end_preserve_other_columns() {
        let mut order = default_order();
        assert!(place_after(&mut order, "produced_iron", Some("gold")));
        let gold_index = order.iter().position(|key| key == "gold").unwrap();
        assert_eq!(order[gold_index + 1], "produced_iron");
        assert!(!place_after(&mut order, "produced_iron", Some("gold")));
        assert!(move_before(&mut order, "gold", None));
        assert_eq!(order.last().map(String::as_str), Some("gold"));
        assert!(!move_before(&mut order, "gold", None));
    }

    #[test]
    fn invalid_moves_do_not_modify_the_order() {
        let mut order = default_order();
        let before = order.clone();
        assert!(!move_before(&mut order, "unknown", Some("gold")));
        assert!(!move_before(&mut order, "gold", Some("unknown")));
        assert!(!move_before(&mut order, "gold", Some("gold")));
        assert!(!place_after(&mut order, "gold", Some("gold")));
        assert_eq!(order, before);
    }

    #[test]
    fn each_visibility_switch_changes_only_its_own_column() {
        for selected in ALL {
            let mut columns = ColumnVisibility::default();
            for column in ALL {
                column.set_enabled(&mut columns, false);
            }
            selected.set_enabled(&mut columns, true);
            assert_eq!(visible_columns(&default_order(), &columns), vec![selected]);
        }
    }

    #[test]
    fn compact_widths_keep_the_approved_digit_budgets() {
        for (column, width) in [
            (ColumnId::Population, 44.0),
            (ColumnId::FoodEffect, 40.0),
            (ColumnId::TaxEffect, 40.0),
            (ColumnId::FearFactor, 30.0),
            (ColumnId::Popularity, 48.0),
            (ColumnId::Gold, 60.0),
            (ColumnId::AcquiredGold, 70.0),
            (ColumnId::HorseArchers, 44.0),
            (ColumnId::Army, 44.0),
            (ColumnId::CamelLancers, 44.0),
            (ColumnId::Shooters, 52.0),
            (ColumnId::Knights, 44.0),
            (ColumnId::ProducedFood, 70.0),
            (ColumnId::ProducedWeapons, 70.0),
            (ColumnId::ProducedStone, 70.0),
            (ColumnId::ProducedIron, 70.0),
            (ColumnId::ProducedWood, 70.0),
            (ColumnId::ArmyKilled, 50.0),
            (ColumnId::ArmyLost, 50.0),
        ] {
            assert_eq!(column.width(16.0), width);
            assert_eq!(column.width(26.0), width * 26.0 / 16.0);
            assert!(column.width(10.0) <= width);
            assert!(column.width(10.0) >= width * 10.0 / 16.0);
            assert!(column.width(10.0) >= 28.0);
        }
        assert_eq!(COLUMN_GAP, 6.0);
    }

    #[test]
    fn short_headers_retain_clear_names_without_changing_saved_ids() {
        assert_eq!(ColumnId::Population.label(), "Pop");
        assert_eq!(ColumnId::Population.full_label(), "Population");
        assert_eq!(ColumnId::Popularity.label(), "Popul.");
        assert_eq!(ColumnId::Popularity.full_label(), "Popularity");
        assert_eq!(ColumnId::AcquiredGold.label(), "Earned");
        assert_eq!(ColumnId::AcquiredGold.key(), "acquired_gold");
        assert_eq!(ColumnId::ArmyKilled.label(), "Kills");
        assert_eq!(ColumnId::ArmyLost.label(), "Losses");
        for column in ALL {
            assert!(!column.label().is_empty());
            assert!(!column.full_label().is_empty());
            assert_eq!(ColumnId::parse(column.key()), Some(column));
        }
    }
}
