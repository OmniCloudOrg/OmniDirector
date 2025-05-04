use rocket::get;
use crate::api::CpiState;
use rocket::response::Responder;

// HTML response for index route
#[derive(Responder)]
#[response(content_type = "text/html")]
pub struct HtmlResponse(String);

// Index route handler - use the same CpiState struct from the main module
#[get("/")]
pub fn index(cpi_state: &rocket::State<CpiState>) -> HtmlResponse {
    // Get version from cargo package
    let version = env!("CARGO_PKG_VERSION");
    let providers = cpi_state.cpi_system.get_providers();
    
    // Create a JSON string of providers for JavaScript
    let providers_json = serde_json::to_string(&providers).unwrap_or_else(|_| "[]".to_string());
    
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CPI System API</title>
    <style>
        :root {{
            --bg-color: #121212;
            --surface-color: #1e1e1e;
            --primary-color: #3644d9;
            --primary-light: #4e59e6;
            --secondary-color: #b066ed;
            --text-color: #e4e4e4;
            --text-secondary: #a0a0a0;
            --success-color: #4caf50;
            --warning-color: #ff9800;
            --danger-color: #f44336;
            --code-bg: #2d2d2d;
            --border-color: #333333;
            --shadow-color: rgba(0, 0, 0, 0.5);
        }}
        
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            line-height: 1.6;
            color: var(--text-color);
            background-color: var(--bg-color);
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }}
        
        h1, h2, h3 {{
            color: var(--text-color);
        }}
        
        .container {{
            background-color: var(--surface-color);
            border-radius: 8px;
            padding: 25px;
            box-shadow: 0 4px 6px var(--shadow-color);
            margin-bottom: 30px;
        }}
        
        code {{
            background-color: var(--code-bg);
            padding: 2px 5px;
            border-radius: 3px;
            font-family: 'Courier New', monospace;
            font-size: 14px;
            color: var(--text-color);
        }}
        
        pre {{
            background-color: var(--code-bg);
            padding: 15px;
            border-radius: 5px;
            overflow-x: auto;
            border-left: 3px solid var(--primary-color);
        }}
        
        .endpoint {{
            margin-bottom: 30px;
            border-left: 4px solid var(--primary-color);
            padding-left: 15px;
            background-color: rgba(54, 68, 217, 0.05);
            padding: 15px;
            border-radius: 0 5px 5px 0;
        }}
        
        .method {{
            display: inline-block;
            padding: 3px 8px;
            border-radius: 3px;
            font-weight: bold;
            margin-right: 10px;
            text-transform: uppercase;
            font-size: 12px;
        }}
        
        .get {{
            background-color: var(--success-color);
            color: white;
        }}
        
        .post {{
            background-color: var(--danger-color);
            color: white;
        }}
        
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 20px;
            background-color: var(--surface-color);
        }}
        
        th, td {{
            padding: 12px 15px;
            text-align: left;
            border-bottom: 1px solid var(--border-color);
        }}
        
        th {{
            background-color: var(--code-bg);
            color: var(--text-color);
            font-weight: 600;
        }}
        
        tr:hover {{
            background-color: rgba(255, 255, 255, 0.05);
        }}
        
        .version {{
            background-color: var(--primary-color);
            color: white;
            padding: 5px 10px;
            border-radius: 20px;
            font-size: 14px;
            font-weight: 500;
            display: inline-block;
            margin-left: 10px;
        }}
        
        button {{
            background-color: var(--primary-color);
            color: white;
            border: none;
            padding: 10px 18px;
            border-radius: 5px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s ease;
            font-weight: 500;
            display: inline-flex;
            align-items: center;
            box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
        }}
        
        button:hover {{
            background-color: var(--primary-light);
            transform: translateY(-2px);
            box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
        }}
        
        button:active {{
            transform: translateY(0);
        }}
        
        /* Modal Styles */
        .modal {{
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0, 0, 0, 0.7);
            z-index: 100;
            overflow: auto;
            backdrop-filter: blur(3px);
        }}
        
        .modal-content {{
            background-color: var(--surface-color);
            margin: 10% auto;
            padding: 25px;
            border-radius: 8px;
            box-shadow: 0 4px 20px var(--shadow-color);
            width: 80%;
            max-width: 600px;
            position: relative;
            border: 1px solid var(--border-color);
            animation: modalFadeIn 0.3s ease-out;
        }}
        
        @keyframes modalFadeIn {{
            from {{ opacity: 0; transform: translateY(-20px); }}
            to {{ opacity: 1; transform: translateY(0); }}
        }}
        
        .close {{
            position: absolute;
            top: 15px;
            right: 20px;
            font-size: 24px;
            font-weight: bold;
            color: var(--text-secondary);
            cursor: pointer;
            transition: color 0.2s;
            width: 30px;
            height: 30px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: 50%;
        }}
        
        .close:hover {{
            color: var(--text-color);
            background-color: rgba(255, 255, 255, 0.1);
        }}
        
        .search-container {{
            margin-bottom: 20px;
            position: relative;
        }}
        
        #providerSearch {{
            width: 100%;
            padding: 12px 15px;
            padding-left: 35px;
            border: 1px solid var(--border-color);
            border-radius: 6px;
            font-size: 15px;
            box-sizing: border-box;
            background-color: var(--code-bg);
            color: var(--text-color);
            transition: all 0.2s;
        }}
        
        #providerSearch:focus {{
            outline: none;
            border-color: var(--primary-color);
            box-shadow: 0 0 0 2px rgba(54, 68, 217, 0.2);
        }}
        
        .search-icon {{
            position: absolute;
            left: 12px;
            top: 50%;
            transform: translateY(-50%);
            color: var(--text-secondary);
            font-size: 16px;
        }}
        
        .provider-list {{
            max-height: 300px;
            overflow-y: auto;
            border: 1px solid var(--border-color);
            padding: 10px;
            border-radius: 6px;
            background-color: var(--code-bg);
            scrollbar-width: thin;
            scrollbar-color: var(--primary-color) var(--code-bg);
        }}
        
        .provider-list::-webkit-scrollbar {{
            width: 8px;
        }}
        
        .provider-list::-webkit-scrollbar-track {{
            background: var(--code-bg);
        }}
        
        .provider-list::-webkit-scrollbar-thumb {{
            background-color: var(--primary-color);
            border-radius: 4px;
        }}
        
        .provider-item {{
            padding: 10px 12px;
            border-bottom: 1px solid var(--border-color);
            transition: background-color 0.2s;
            cursor: pointer;
        }}
        
        .provider-item:hover {{
            background-color: rgba(54, 68, 217, 0.1);
        }}
        
        .provider-item:last-child {{
            border-bottom: none;
        }}
        
        .count-badge {{
            background-color: var(--primary-color);
            color: white;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 12px;
            margin-left: 8px;
            font-weight: bold;
        }}
        
        /* Responsive adjustments */
        @media (max-width: 768px) {{
            .modal-content {{
                width: 90%;
                margin: 20% auto;
            }}
            
            body {{
                padding: 10px;
            }}
            
            .container {{
                padding: 15px;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>CPI System API <span class="version">v{version}</span></h1>
        
        <p>Welcome to the CPI System API. This API provides access to Cross-Provider Integration (CPI) 
           functionality through a RESTful interface.</p>
        
        <h2>Available Providers <span class="count-badge">{}</span></h2>
        <p><button id="viewProvidersBtn">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" style="margin-right: 8px; fill: currentColor;">
                <path d="M12 9a3 3 0 100 6 3 3 0 000-6zM3 9a3 3 0 100 6 3 3 0 000-6zM21 9a3 3 0 100 6 3 3 0 000-6z"/>
            </svg>
            View Providers
        </button></p>
        
        <!-- Providers Modal -->
        <div id="providersModal" class="modal">
            <div class="modal-content">
                <span class="close">&times;</span>
                <h2>Available Providers</h2>
                <div class="search-container">
                    <span class="search-icon">
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="11" cy="11" r="8"></circle>
                            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                        </svg>
                    </span>
                    <input type="text" id="providerSearch" placeholder="Search providers...">
                </div>
                <div class="provider-list" id="providerList">
                    <!-- Provider items will be dynamically inserted here -->
                </div>
            </div>
        </div>
        
        <h2>API Endpoints</h2>
        
        <div class="endpoint">
            <h3><span class="method get">GET</span> /</h3>
            <p>This documentation page</p>
        </div>

        <div class="endpoint">
            <h3><span class="method get">GET</span> /providers</h3>
            <p>Returns a list of all available providers</p>
            <pre><code>GET /providers
Response: ["provider1", "provider2", ...]</code></pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method get">GET</span> /actions</h3>
            <p>Returns a list of all unique actions across all providers</p>
            <pre><code>GET /actions
Response: ["action1", "action2", ...]</code></pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method get">GET</span> /actions/:provider</h3>
            <p>Returns a list of actions available for a specific provider</p>
            <pre><code>GET /actions/provider1
Response: ["action1", "action2", ...]</code></pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method get">GET</span> /params/:provider/:action</h3>
            <p>Returns a list of required parameters for a specific action</p>
            <pre><code>GET /params/provider1/action1
Response: ["param1", "param2", ...]</code></pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method post">POST</span> /action</h3>
            <p>Executes a provider action with the given parameters</p>
            <pre><code>POST /action
Content-Type: application/json

{{
  "provider": "provider1",
  "action": "action1",
  "params": {{
    "param1": "value1",
    "param2": "value2"
  }}
}}

Response:
{{
  "success": true,
  "result": {{ ... }},
  "error": null
}}</code></pre>
        </div>
    </div>

    <script>
        // Store providers data
        const providers = {providers_json};
        
        // Get modal elements
        const modal = document.getElementById('providersModal');
        const btn = document.getElementById('viewProvidersBtn');
        const span = document.getElementsByClassName('close')[0];
        const searchInput = document.getElementById('providerSearch');
        const providerList = document.getElementById('providerList');
        
        // Populate provider list
        function populateProviders(filter = '') {{
            providerList.innerHTML = '';
            
            const filteredProviders = providers.filter(provider => 
                provider.toLowerCase().includes(filter.toLowerCase())
            );
            
            if (filteredProviders.length === 0) {{
                providerList.innerHTML = '<div class="provider-item">No providers found</div>';
                return;
            }}
            
            filteredProviders.forEach(provider => {{
                const item = document.createElement('div');
                item.className = 'provider-item';
                item.textContent = provider;
                providerList.appendChild(item);
            }});
        }}
        
        // Initialize provider list
        populateProviders();
        
        // Search functionality
        searchInput.addEventListener('input', function() {{
            populateProviders(this.value);
        }});
        
        // Open modal
        btn.onclick = function() {{
            modal.style.display = 'block';
            searchInput.focus();
        }}
        
        // Close modal
        span.onclick = function() {{
            modal.style.display = 'none';
        }}
        
        // Close modal when clicking outside of it
        window.onclick = function(event) {{
            if (event.target === modal) {{
                modal.style.display = 'none';
            }}
        }}
        
        // Close modal with Escape key
        document.addEventListener('keydown', function(event) {{
            if (event.key === 'Escape' && modal.style.display === 'block') {{
                modal.style.display = 'none';
            }}
        }});
    </script>
</body>
</html>
"#,
        providers.len(),
    );

    HtmlResponse(html)
}