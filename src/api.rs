use reqwest::Client;
use serde::{Deserialize, Serialize};


const BASE_URL: &str = "https://world.openfoodfacts.org/api/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub code: String,
    pub product_name: Option<String>,
    pub brands: Option<String>,
    pub nutriscore_grade: Option<String>,
    pub nova_group: Option<i32>,
    pub additives_tags: Option<Vec<String>>,
    pub nutriments: Option<Nutriments>,
    pub ingredients_text: Option<String>,
    pub categories: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Nutriments {
    #[serde(default)]
    pub energy_kcal_100g: Option<f64>,
    #[serde(default)]
    pub fat_100g: Option<f64>,
    #[serde(default)]
    pub saturated_fat_100g: Option<f64>,
    #[serde(default)]
    pub sugars_100g: Option<f64>,
    #[serde(default)]
    pub salt_100g: Option<f64>,
    #[serde(default)]
    pub proteins_100g: Option<f64>,
    #[serde(default)]
    pub fiber_100g: Option<f64>,
    #[serde(default)]
    pub carbohydrates_100g: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    #[allow(dead_code)] pub count: u64,
    pub products: Vec<Product>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ProductResponse {
    pub status: u8,
    pub product: Option<Product>,
}

pub struct OpenFoodFactsApi {
    client: Client,
}

impl OpenFoodFactsApi {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("NutriScan/0.1.0 (https://github.com/quantumnic/nutriscan)")
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub async fn search(&self, query: &str, page_size: u32) -> Result<Vec<Product>, ApiError> {
        let url = format!(
            "{}/search?search_terms={}&page_size={}&json=1",
            BASE_URL,
            urlencoded(query),
            page_size
        );
        let resp: SearchResponse = self.client.get(&url).send().await?.json().await?;
        Ok(resp.products)
    }

    #[allow(dead_code)]
    pub async fn get_by_barcode(&self, barcode: &str) -> Result<Option<Product>, ApiError> {
        let url = format!("{}/product/{}.json", BASE_URL, barcode);
        let resp: ProductResponse = self.client.get(&url).send().await?.json().await?;
        if resp.status == 1 {
            Ok(resp.product)
        } else {
            Ok(None)
        }
    }
}

fn urlencoded(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[derive(Debug)]
pub enum ApiError {
    Http(reqwest::Error),
    #[allow(dead_code)]
    #[allow(dead_code)] Parse(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Http(e) => write!(f, "HTTP error: {}", e),
            ApiError::Parse(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Http(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoded_simple() {
        assert_eq!(urlencoded("hello"), "hello");
    }

    #[test]
    fn test_urlencoded_spaces() {
        assert_eq!(urlencoded("coca cola"), "coca+cola");
    }

    #[test]
    fn test_urlencoded_special() {
        assert_eq!(urlencoded("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn test_api_new() {
        let _api = OpenFoodFactsApi::new();
    }

    #[test]
    fn test_product_deserialize() {
        let json = r#"{
            "code": "123",
            "product_name": "Test",
            "brands": "Brand",
            "nutriscore_grade": "a",
            "nova_group": 1,
            "additives_tags": ["en:e100"],
            "nutriments": {"energy_kcal_100g": 100.0},
            "ingredients_text": "water",
            "categories": "beverages",
            "image_url": null
        }"#;
        let p: Product = serde_json::from_str(json).unwrap();
        assert_eq!(p.code, "123");
        assert_eq!(p.product_name.unwrap(), "Test");
        assert_eq!(p.nova_group.unwrap(), 1);
    }

    #[test]
    fn test_nutriments_default() {
        let json = r#"{}"#;
        let n: Nutriments = serde_json::from_str(json).unwrap();
        assert!(n.energy_kcal_100g.is_none());
    }
}
