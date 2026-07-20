use serde_json::Value;

fn geocode(location: &str) -> Result<(f64, f64), String> {
    if let Some((lat_str, lon_str)) = location.split_once(',') {
        if let (Ok(lat), Ok(lon)) = (lat_str.trim().parse::<f64>(), lon_str.trim().parse::<f64>()) {
            return Ok((lat, lon));
        }
    }
    
    let query = if location.len() == 5 && location.chars().all(|c| c.is_ascii_digit()) {
        format!("{} US", location)
    } else {
        location.to_string()
    };
    
    // We must manually URL encode the query. But since we don't have url-encoding crate imported by default, 
    // let's do a basic replace for spaces.
    let query_encoded = query.replace(" ", "%20");
    let url = format!("https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1", query_encoded);
    
    let req = match ureq::get(&url).set("User-Agent", "FastMD Weather Tool/1.0").call() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.geocode.api_failed", error = %e, url = %url, "Nominatim geocoding API request failed. Operator should verify network or API limits.");
            return Err(format!("Nominatim API error: {}", e));
        }
    };
    
    let json: Value = match req.into_json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.geocode.json_failed", error = %e, "Nominatim geocoding API returned invalid JSON. Operator should verify API response.");
            return Err(format!("Nominatim JSON error: {}", e));
        }
    };
    
    let first = json.as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| {
            tracing::error!(name = "tool.weather.geocode.not_found", location = %location, "Nominatim geocoding API returned no results. Operator should verify location name.");
            "Location not found".to_string()
        })?;
        
    let lat = first.get("lat").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).ok_or_else(|| {
        tracing::error!(name = "tool.weather.geocode.missing_lat", location = %location, "Nominatim geocoding API response missing latitude.");
        "Missing lat".to_string()
    })?;
    let lon = first.get("lon").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()).ok_or_else(|| {
        tracing::error!(name = "tool.weather.geocode.missing_lon", location = %location, "Nominatim geocoding API response missing longitude.");
        "Missing lon".to_string()
    })?;
    
    Ok((lat, lon))
}

pub fn tool_get_weather(location: &str, date_range: Option<&str>) -> Result<crate::tools::dtos::GetWeatherResponse, String> {
    // Reference: https://www.weather.gov/documentation/services-web-api
    
    let (lat, lon) = match geocode(location) {
        Ok(coords) => coords,
        Err(e) => return Err(e),
    };
    
    let points_url = format!("https://api.weather.gov/points/{},{}", lat, lon);
    let req = match ureq::get(&points_url).set("User-Agent", "FastMD Weather Tool/1.0").call() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.points_api_failed", error = %e, url = %points_url, "NWS Points API request failed. Operator should verify network connectivity.");
            return Err(format!("NWS Points API error: {}", e));
        }
    };
    
    let json: Value = match req.into_json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.points_json_failed", error = %e, "NWS Points API returned invalid JSON. Operator should verify API status.");
            return Err(format!("NWS Points JSON error: {}", e));
        }
    };
    
    let forecast_url = match json.get("properties").and_then(|p| p.get("forecast")).and_then(|f| f.as_str()) {
        Some(url) => url,
        None => return Err("Could not find forecast URL in NWS response".to_string()),
    };
    
    let req = match ureq::get(forecast_url).set("User-Agent", "FastMD Weather Tool/1.0").call() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.forecast_api_failed", error = %e, url = %forecast_url, "NWS Forecast API request failed. Operator should verify network connectivity.");
            return Err(format!("NWS Forecast API error: {}", e));
        }
    };
    
    let forecast_json: Value = match req.into_json() {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(name = "tool.weather.nws.forecast_json_failed", error = %e, "NWS Forecast API returned invalid JSON. Operator should verify API status.");
            return Err(format!("NWS Forecast JSON error: {}", e));
        }
    };
    
    let periods = match forecast_json.get("properties").and_then(|p| p.get("periods")).and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return Err("Could not find periods in NWS forecast response".to_string()),
    };
    
    let mut results = Vec::new();
    
    let dr = date_range.unwrap_or("").to_lowercase();
    let filter_dr = !dr.is_empty();
    
    for period in periods {
        let start = period.get("startTime").and_then(|v| v.as_str()).unwrap_or("");
        
        // Simple string containment check for date range (e.g. if dr is "2026-07-18")
        // Or if it's not filtered, just add it.
        if !filter_dr || (start.contains(&dr) || (start.len() >= 10 && dr.contains(&start[..10]))) {
            let name = period.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let temp = period.get("temperature").and_then(|v| v.as_i64()).unwrap_or(0);
            let temp_unit = period.get("temperatureUnit").and_then(|v| v.as_str()).unwrap_or("");
            let forecast = period.get("detailedForecast").and_then(|v| v.as_str()).unwrap_or("");
            
            results.push(serde_json::json!({
                "period_name": name,
                "start_time": start,
                "temperature": format!("{} {}", temp, temp_unit),
                "detailed_forecast": forecast
            }));
        }
    }
    
    if results.is_empty() {
        let err_msg = if filter_dr {
            format!("No weather data found matching date range '{}'. Remember NWS only provides ~7 days of forecast.", dr)
        } else {
            "No forecast periods found.".to_string()
        };
        tracing::warn!(name = "tool.weather.no_results", location = %location, "No weather data found for the given location and date range. Operator should verify location query.");
        return Err(err_msg);
    }
    
    Ok(crate::tools::dtos::GetWeatherResponse {
        result: serde_json::to_string(&results).unwrap_or_default()
    })
}
