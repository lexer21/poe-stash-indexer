use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct StashTabResponse {
    pub next_change_id: String,
    pub stashes: Vec<Stash>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Stash {
    #[serde(rename(deserialize = "accountName"))]
    pub account_name: Option<String>,
    #[serde(rename(deserialize = "lastCharacterName"))]
    pub last_character_name: Option<String>,
    pub id: String,
    pub stash: Option<String>,
    #[serde(rename(deserialize = "stashType"))]
    pub stash_type: String,
    pub items: Vec<Item>,
    pub public: bool,
    pub league: Option<String>,
}

impl Stash {
    pub fn is_standard(&self) -> bool {
        self.league
            .as_ref()
            .map_or(false, |l| l.eq("Standard") || l.eq("Hardcore"))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Item {
    pub name: String,
    pub id: String,
    #[serde(rename(deserialize = "inventoryId"))]
    pub inventory_name: Option<String>,
    pub note: Option<String>,
    #[serde(rename(deserialize = "typeLine"))]
    pub type_line: String,
    #[serde(rename(deserialize = "stackSize"))]
    pub stack_size: Option<u32>,
    pub extended: ItemExtendedProp,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ItemExtendedProp {
    pub category: String,
    #[serde(rename(deserialize = "baseType"))]
    pub base_type: String,
}