use std::time::Duration;

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_API_KEY: &str = "fef1285fb85a74350b3292b5fac37fce";

#[tokio::main]
async fn main() {
    // Test with the actual volume name from the DVD
    let volume_name = "FOSTERS_DISC_ONE";
    
    println!("Testing TMDB API with volume name: '{}'", volume_name);
    println!("API Key: {}...", &TMDB_API_KEY[..10]);
    
    let client = reqwest::Client::builder()
        .user_agent("Ripley/0.1.0")
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    
    // Try searching for TV show
    println!("\n=== Searching TV Shows ===");
    let tv_url = format!(
        "{}/search/tv?api_key={}&query={}",
        TMDB_API_BASE,
        TMDB_API_KEY,
        urlencoding::encode(volume_name)
    );
    println!("URL: {}", tv_url.replace(TMDB_API_KEY, "***"));
    
    match client.get(&tv_url).send().await {
        Ok(response) => {
            println!("Status: {}", response.status());
            match response.text().await {
                Ok(text) => {
                    println!("Response:\n{}", text);
                }
                Err(e) => println!("Error reading response: {}", e),
            }
        }
        Err(e) => println!("Error sending request: {}", e),
    }
    
    // Try with a cleaned up version
    println!("\n=== Trying cleaned up name: 'Foster's Home for Imaginary Friends' ===");
    let clean_name = "Foster's Home for Imaginary Friends";
    let tv_url2 = format!(
        "{}/search/tv?api_key={}&query={}",
        TMDB_API_BASE,
        TMDB_API_KEY,
        urlencoding::encode(clean_name)
    );
    println!("URL: {}", tv_url2.replace(TMDB_API_KEY, "***"));
    
    match client.get(&tv_url2).send().await {
        Ok(response) => {
            println!("Status: {}", response.status());
            match response.text().await {
                Ok(text) => {
                    println!("Response:\n{}", text);
                }
                Err(e) => println!("Error reading response: {}", e),
            }
        }
        Err(e) => println!("Error sending request: {}", e),
    }
}
