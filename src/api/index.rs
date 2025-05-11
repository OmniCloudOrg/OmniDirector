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
r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CPI System API</title>
    <!-- Tailwind Play CDN -->
    <script src="https://cdn.tailwindcss.com"></script>
    <script>
        tailwind.config = {{
            darkMode: 'class',
            theme: {{
                extend: {{
                    colors: {{
                        primary: {{
                            DEFAULT: '#3644d9',
                            light: '#4e59e6'
                        }},
                        secondary: '#b066ed',
                        dark: {{
                            bg: '#121212',
                            surface: '#1e1e1e',
                            code: '#2d2d2d',
                            border: '#333333'
                        }}
                    }}
                }}
            }}
        }}
    </script>
</head>
<body class="bg-dark-bg text-gray-200 font-sans leading-relaxed">
    <div class="max-w-7xl mx-auto p-4 md:p-6">
        <!-- Main Container -->
        <div class="bg-dark-surface rounded-lg p-6 shadow-lg mb-8">
            <h1 class="text-2xl md:text-3xl font-bold flex items-center">
                CPI System API 
                <span class="bg-primary text-white text-sm py-1 px-3 rounded-full ml-3">v{}</span>
            </h1>
            
            <p class="my-4">
                Welcome to the CPI System API. This API provides access to Cross-Provider Integration (CPI) 
                functionality through a RESTful interface.
            </p>
            
            <div class="flex items-center mb-6">
                <h2 class="text-xl font-semibold">Available Providers 
                    <span class="bg-primary text-white text-xs py-0.5 px-2 rounded-full ml-2">{}</span>
                </h2>
                <div class="ml-4">
                    <button id="viewProvidersBtn" class="bg-primary hover:bg-primary-light text-white py-2 px-4 rounded flex items-center transition-all duration-200 shadow hover:shadow-md hover:-translate-y-0.5">
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" class="mr-2 fill-current">
                            <path d="M12 9a3 3 0 100 6 3 3 0 000-6zM3 9a3 3 0 100 6 3 3 0 000-6zM21 9a3 3 0 100 6 3 3 0 000-6z"/>
                        </svg>
                        View Providers
                    </button>
                </div>
            </div>
            
            <h2 class="text-xl font-semibold mb-4">API Endpoints</h2>
            
            <!-- Endpoints -->
            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-green-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">GET</span>
                    /
                </h3>
                <p>This documentation page</p>
            </div>

            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-green-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">GET</span>
                    /providers
                </h3>
                <p>Returns a list of all available providers</p>
                <pre class="bg-dark-code p-4 rounded mt-2 overflow-x-auto border-l-2 border-primary"><code>GET /providers
Response: ["provider1", "provider2", ...]</code></pre>
            </div>
            
            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-green-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">GET</span>
                    /actions
                </h3>
                <p>Returns a list of all unique actions across all providers</p>
                <pre class="bg-dark-code p-4 rounded mt-2 overflow-x-auto border-l-2 border-primary"><code>GET /actions
Response: ["action1", "action2", ...]</code></pre>
            </div>
            
            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-green-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">GET</span>
                    /actions/:provider
                </h3>
                <p>Returns a list of actions available for a specific provider</p>
                <pre class="bg-dark-code p-4 rounded mt-2 overflow-x-auto border-l-2 border-primary"><code>GET /actions/provider1
Response: ["action1", "action2", ...]</code></pre>
            </div>
            
            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-green-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">GET</span>
                    /params/:provider/:action
                </h3>
                <p>Returns a list of required parameters for a specific action</p>
                <pre class="bg-dark-code p-4 rounded mt-2 overflow-x-auto border-l-2 border-primary"><code>GET /params/provider1/action1
Response: ["param1", "param2", ...]</code></pre>
            </div>
            
            <div class="mb-6 border-l-4 border-primary pl-4 rounded-r-md bg-opacity-5 bg-primary p-4">
                <h3 class="flex items-center mb-2">
                    <span class="bg-red-600 text-white px-2 py-0.5 rounded text-xs font-bold uppercase mr-2">POST</span>
                    /action
                </h3>
                <p>Executes a provider action with the given parameters</p>
                <pre class="bg-dark-code p-4 rounded mt-2 overflow-x-auto border-l-2 border-primary"><code>POST /action
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
    </div>

    <!-- Three-column Provider Explorer Modal -->
    <div id="providersModal" class="fixed inset-0 bg-black bg-opacity-70 z-50 hidden backdrop-blur-sm flex items-center justify-center">
        <div class="bg-dark-surface w-11/12 max-w-[80vw] rounded-lg shadow-lg p-6 mx-4 my-8 animate-fade-in max-h-[90vh] flex flex-col">
            <!-- Modal header -->
            <div class="flex justify-between items-center mb-4">
                <h2 class="text-xl font-semibold">CPI System Explorer</h2>
                <button id="closeModal" class="text-gray-400 hover:text-white hover:bg-opacity-10 hover:bg-white w-8 h-8 rounded-full flex items-center justify-center transition-colors">
                    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <line x1="18" y1="6" x2="6" y2="18"></line>
                        <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                </button>
            </div>
            
            <!-- Three-column layout -->
            <div class="flex-1 overflow-hidden flex flex-row gap-4" style="min-height: 500px">
                <!-- Column 1: Providers -->
                <div class="w-1/3 flex flex-col bg-dark-code rounded-lg p-4 border border-dark-border">
                    <h3 class="text-lg font-medium mb-3">Providers</h3>
                    
                    <!-- Search providers -->
                    <div class="relative mb-3">
                        <div class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <circle cx="11" cy="11" r="8"></circle>
                                <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                            </svg>
                        </div>
                        <input type="text" id="providerSearch" placeholder="Search providers..." 
                            class="w-full py-2 pl-10 pr-4 bg-dark-bg border border-dark-border rounded-md text-gray-200 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary transition-colors text-sm">
                    </div>
                    
                    <!-- Provider list -->
                    <div id="providerList" class="flex-1 overflow-y-auto border border-dark-border rounded-md bg-dark-bg p-2 scrollbar scrollbar-thin scrollbar-thumb-primary scrollbar-track-dark-code">
                        <!-- Provider items will be inserted here by JS -->
                    </div>
                </div>
                
                <!-- Column 2: Actions -->
                <div class="w-1/3 flex flex-col bg-dark-code rounded-lg p-4 border border-dark-border">
                    <h3 class="text-lg font-medium mb-3">Actions</h3>
                    
                    <!-- Search actions - new feature -->
                    <div class="relative mb-3">
                        <div class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <circle cx="11" cy="11" r="8"></circle>
                                <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                            </svg>
                        </div>
                        <input type="text" id="actionSearch" placeholder="Search actions..." 
                            class="w-full py-2 pl-10 pr-4 bg-dark-bg border border-dark-border rounded-md text-gray-200 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary transition-colors text-sm">
                    </div>
                    
                    <!-- Selected provider info -->
                    <div id="selectedProviderInfo" class="mb-3 px-3 py-2 bg-primary bg-opacity-10 rounded-md border border-primary border-opacity-30 hidden">
                        <div class="flex items-center">
                            <span class="bg-primary bg-opacity-20 text-primary-light px-2 py-1 rounded text-xs font-medium mr-2">Provider</span>
                            <span id="selectedProviderName" class="text-sm font-medium truncate">Name</span>
                        </div>
                    </div>
                    
                    <!-- Actions list - keep same ID for compatibility -->
                    <div id="providerActions" class="flex-1 overflow-y-auto border border-dark-border rounded-md bg-dark-bg p-2 scrollbar scrollbar-thin scrollbar-thumb-primary scrollbar-track-dark-code">
                        <p class="text-gray-400 italic text-sm p-2">Select a provider to view actions</p>
                    </div>
                </div>
                
                <!-- Column 3: Action Details -->
                <div class="w-1/3 flex flex-col bg-dark-code rounded-lg p-4 border border-dark-border">
                    <h3 class="text-lg font-medium mb-3">Action Details</h3>
                    
                    <!-- Selected action info -->
                    <div id="selectedActionInfo" class="mb-3 px-3 py-2 bg-secondary bg-opacity-10 rounded-md border border-secondary border-opacity-30 hidden">
                        <div class="flex items-center">
                            <span class="bg-secondary bg-opacity-20 text-secondary px-2 py-1 rounded text-xs font-medium mr-2">Action</span>
                            <span id="selectedActionName" class="text-sm font-medium truncate">Name</span>
                        </div>
                    </div>
                    
                    <!-- Provider details container - keep same ID for compatibility -->
                    <div id="providerDetails" class="hidden flex-1 overflow-hidden flex flex-col">
                        <!-- Parameters section - keep same ID for compatibility -->
                        <div id="actionDetails" class="hidden flex-1 overflow-hidden flex flex-col">
                            <h4 class="text-sm font-medium text-gray-400 mb-2">Required Parameters</h4>
                            <div id="actionParams" class="bg-dark-bg rounded-md p-3 border border-dark-border overflow-y-auto mb-4 max-h-32">
                                <p class="text-gray-400 italic text-sm">Select an action to view parameters</p>
                            </div>
                            
                            <h4 class="text-sm font-medium text-gray-400 mb-2">Test Action</h4>
                            <div id="actionParamInputs" class="bg-dark-bg rounded-md p-3 border border-dark-border overflow-y-auto mb-3 flex-1">
                                <p class="text-gray-400 italic text-sm">Select an action to test</p>
                            </div>
                            
                            <button id="executeActionBtn" class="bg-primary hover:bg-primary-light text-white py-2 px-4 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed mb-3" disabled>
                                Execute Action
                            </button>
                            
                            <div id="actionResults" class="hidden">
                                <h4 class="text-sm font-medium text-gray-400 mb-2">Results</h4>
                                <pre id="actionResultsData" class="bg-dark-bg p-3 rounded border border-dark-border overflow-y-auto overflow-x-auto max-h-40 text-sm"></pre>
                            </div>
                        </div>
                    </div>
                    
                    <!-- No action selected state -->
                    <div id="noActionSelected" class="flex-1 flex items-center justify-center">
                        <p class="text-gray-400 italic text-sm">Select an action to view details</p>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <script>
        // Store providers data
        const providers = {providers_json};
        
        // DOM elements
        const modal = document.getElementById('providersModal');
        const viewProvidersBtn = document.getElementById('viewProvidersBtn');
        const closeModalBtn = document.getElementById('closeModal');
        const searchInput = document.getElementById('providerSearch');
        const providerList = document.getElementById('providerList');
        const providerDetails = document.getElementById('providerDetails');
        const selectedProviderName = document.getElementById('selectedProviderName');
        const providerActions = document.getElementById('providerActions');
        const actionDetails = document.getElementById('actionDetails');
        const actionParams = document.getElementById('actionParams');
        const actionParamInputs = document.getElementById('actionParamInputs');
        const executeActionBtn = document.getElementById('executeActionBtn');
        const actionResults = document.getElementById('actionResults');
        const actionResultsData = document.getElementById('actionResultsData');
        
        // New UI elements
        const actionSearch = document.getElementById('actionSearch');
        const selectedProviderInfo = document.getElementById('selectedProviderInfo');
        const selectedActionInfo = document.getElementById('selectedActionInfo');
        const selectedActionName = document.getElementById('selectedActionName');
        const noActionSelected = document.getElementById('noActionSelected');
        
        // Helper for creating HTML elements
        function createElement(tag, className, innerHTML = '') {{
            const element = document.createElement(tag);
            if (className) element.className = className;
            if (innerHTML) element.innerHTML = innerHTML;
            return element;
        }}
        
        // Populate provider list
        function populateProviders(filter = '') {{
            providerList.innerHTML = '';
            
            const filteredProviders = providers.filter(provider => 
                provider.toLowerCase().includes(filter.toLowerCase())
            );
            
            if (filteredProviders.length === 0) {{
                providerList.appendChild(
                    createElement('div', 'p-3 text-gray-400 italic', 'No providers found')
                );
                return;
            }}
            
            // Store all provider buttons
            window.providerButtons = [];
            
            filteredProviders.forEach(provider => {{
                const item = createElement(
                    'div', 
                    'p-3 border-b border-dark-border hover:bg-primary hover:bg-opacity-10 cursor-pointer transition-colors rounded',
                    provider
                );
                
                item.addEventListener('click', () => {{
                    // Highlight selected provider
                    window.providerButtons.forEach(btn => {{
                        btn.classList.remove('border-primary', 'bg-primary', 'bg-opacity-10');
                    }});
                    item.classList.add('border-primary', 'bg-primary', 'bg-opacity-10');
                    
                    loadProviderDetails(provider);
                }});
                
                window.providerButtons.push(item);
                providerList.appendChild(item);
            }});
        }}
        
        // Load provider details - modified for three-column layout
        async function loadProviderDetails(provider) {{
            // Update selected provider UI
            selectedProviderInfo.classList.remove('hidden');
            selectedProviderName.textContent = provider;
            providerDetails.classList.remove('hidden');
            
            // Reset action selection
            selectedActionInfo.classList.add('hidden');
            actionDetails.classList.add('hidden');
            if (noActionSelected) noActionSelected.classList.remove('hidden');
            
            // Clear previous state
            providerActions.innerHTML = '<p class="text-gray-400 italic text-sm p-2">Loading actions...</p>';
            
            try {{
                // Fetch provider actions
                const response = await fetch(`/actions/${{provider}}`);
                if (!response.ok) throw new Error('Failed to fetch actions');
                
                const actions = await response.json();
                
                // Display actions
                providerActions.innerHTML = '';
                
                if (actions.length === 0) {{
                    providerActions.innerHTML = '<p class="text-gray-400 italic text-sm p-2">No actions available</p>';
                    return;
                }}
                
                // Create and store all action buttons
                window.actionButtons = [];
                
                actions.forEach(action => {{
                    const actionBtn = createElement(
                        'button',
                        'block w-full text-left py-2 px-3 rounded bg-dark-bg border border-dark-border hover:border-secondary hover:bg-opacity-5 hover:bg-secondary transition-colors mb-2 text-sm',
                        action
                    );
                    
                    actionBtn.addEventListener('click', () => {{
                        // Highlight selected action
                        window.actionButtons.forEach(btn => {{
                            btn.classList.remove('border-secondary', 'bg-secondary', 'bg-opacity-10');
                        }});
                        actionBtn.classList.add('border-secondary', 'bg-secondary', 'bg-opacity-10');
                        
                        // Update selected action UI
                        selectedActionInfo.classList.remove('hidden');
                        selectedActionName.textContent = action;
                        if (noActionSelected) noActionSelected.classList.add('hidden');
                        
                        // Load action parameters
                        loadActionParams(provider, action);
                    }});
                    
                    window.actionButtons.push(actionBtn);
                    providerActions.appendChild(actionBtn);
                }});
                
                // Set up action search functionality
                if (actionSearch) {{
                    actionSearch.value = '';
                    actionSearch.onkeyup = () => filterActions(actionSearch.value);
                }}
                
            }} catch (error) {{
                providerActions.innerHTML = `<p class="text-red-400 text-sm p-2">Error: ${{error.message}}</p>`;
            }}
        }}
        
        // New function to filter actions
        function filterActions(filter = '') {{
            if (!window.actionButtons) return;
            
            const filteredCount = window.actionButtons.reduce((count, btn) => {{
                const actionName = btn.textContent.trim();
                const isVisible = actionName.toLowerCase().includes(filter.toLowerCase());
                btn.style.display = isVisible ? 'block' : 'none';
                return count + (isVisible ? 1 : 0);
            }}, 0);
            
            // Show message if no actions match filter
            if (filteredCount === 0 && window.actionButtons.length > 0) {{
                const noResults = document.createElement('p');
                noResults.className = 'text-gray-400 italic text-sm p-2';
                noResults.textContent = 'No actions match your search';
                providerActions.appendChild(noResults);
            }}
        }}
        
        // Load action parameters
        async function loadActionParams(provider, action) {{
            // Show action details section
            actionDetails.classList.remove('hidden');
            actionParams.innerHTML = '<p class="text-gray-400 italic text-sm">Loading parameters...</p>';
            actionParamInputs.innerHTML = '';
            actionResults.classList.add('hidden');
            
            try {{
                // Fetch action parameters
                const response = await fetch(`/params/${{provider}}/${{action}}`);
                if (!response.ok) throw new Error('Failed to fetch parameters');
                
                const params = await response.json();
                
                // Display parameters
                actionParams.innerHTML = '';
                
                if (params.length === 0) {{
                    actionParams.innerHTML = '<p class="text-gray-400 italic text-sm">No parameters required</p>';
                    
                    // Enable execute button if no params are needed
                    executeActionBtn.disabled = false;
                    
                    // Simple form for no parameters
                    actionParamInputs.innerHTML = '<p class="text-gray-400 text-sm">This action does not require any parameters.</p>';
                }} else {{
                    // Create parameter list
                    const paramList = createElement('ul', 'list-disc pl-5 space-y-1');
                    params.forEach(param => {{
                        paramList.appendChild(
                            createElement('li', 'text-sm', `<code class="bg-dark-bg px-1.5 py-0.5 rounded">${{param}}</code>`)
                        );
                    }});
                    actionParams.appendChild(paramList);
                    
                    // Create input fields for parameters
                    actionParamInputs.innerHTML = '';
                    params.forEach(param => {{
                        const inputGroup = createElement('div', 'mb-3');
                        
                        const label = createElement('label', 'block text-sm font-medium text-gray-300 mb-1', param);
                        label.setAttribute('for', `param-${{param}}`);
                        
                        const input = createElement('input', 'w-full bg-dark-bg border border-dark-border rounded py-2 px-3 text-gray-200 focus:outline-none focus:border-secondary transition-colors text-sm');
                        input.setAttribute('id', `param-${{param}}`);
                        input.setAttribute('name', param);
                        input.setAttribute('placeholder', `Enter ${{param}} value...`);
                        
                        inputGroup.appendChild(label);
                        inputGroup.appendChild(input);
                        actionParamInputs.appendChild(inputGroup);
                    }});
                    
                    // Enable execute button when parameters are loaded
                    executeActionBtn.disabled = false;
                }}
                
                // Set up execute button
                executeActionBtn.onclick = () => executeAction(provider, action, params);
                
            }} catch (error) {{
                actionParams.innerHTML = `<p class="text-red-400 text-sm">Error: ${{error.message}}</p>`;
                actionParamInputs.innerHTML = `<p class="text-red-400 text-sm">Could not load parameters: ${{error.message}}</p>`;
                executeActionBtn.disabled = true;
            }}
        }}
        
        // Execute action
        async function executeAction(provider, action, paramsList) {{
            actionResults.classList.add('hidden');
            executeActionBtn.disabled = true;
            executeActionBtn.innerHTML = '<span class="inline-block animate-spin mr-2">↻</span> Executing...';
            
            // Collect parameter values
            const params = {{}};
            paramsList.forEach(param => {{
                const input = document.getElementById(`param-${{param}}`);
                params[param] = input ? input.value : '';
            }});
            
            try {{
                // Execute action API call
                const response = await fetch('/action', {{
                    method: 'POST',
                    headers: {{
                        'Content-Type': 'application/json',
                    }},
                    body: JSON.stringify({{
                        provider,
                        action,
                        params
                    }}),
                }});
                
                const result = await response.json();
                
                // Display results
                actionResults.classList.remove('hidden');
                actionResultsData.textContent = JSON.stringify(result, null, 2);
                
                // Format result based on success/failure
                if (result.success) {{
                    actionResultsData.classList.remove('border-red-500');
                    actionResultsData.classList.add('border-green-500');
                }} else {{
                    actionResultsData.classList.remove('border-green-500');
                    actionResultsData.classList.add('border-red-500');
                }}
                
            }} catch (error) {{
                actionResults.classList.remove('hidden');
                actionResultsData.textContent = `Error: ${{error.message}}`;
                actionResultsData.classList.remove('border-green-500');
                actionResultsData.classList.add('border-red-500');
            }} finally {{
                executeActionBtn.disabled = false;
                executeActionBtn.innerHTML = 'Execute Action';
            }}
        }}
        
        // Initialize UI when opening modal
        viewProvidersBtn.onclick = function() {{
            modal.classList.remove('hidden');
            
            // Reset UI state
            if (selectedProviderInfo) selectedProviderInfo.classList.add('hidden');
            if (selectedActionInfo) selectedActionInfo.classList.add('hidden');
            if (noActionSelected) noActionSelected.classList.remove('hidden');
            actionDetails.classList.add('hidden');
            
            // Populate providers
            populateProviders();
            
            // Focus on search
            searchInput.value = '';
            if (actionSearch) actionSearch.value = '';
            searchInput.focus();
        }}
        
        // Search functionality
        searchInput.addEventListener('input', function() {{
            populateProviders(this.value);
        }});
        
        // Modal controls
        closeModalBtn.onclick = function() {{
            modal.classList.add('hidden');
        }}
        
        // Close modal when clicking outside
        modal.addEventListener('click', function(event) {{
            if (event.target === modal) {{
                modal.classList.add('hidden');
            }}
        }});
        
        // Close modal with Escape key
        document.addEventListener('keydown', function(event) {{
            if (event.key === 'Escape' && !modal.classList.contains('hidden')) {{
                modal.classList.add('hidden');
            }}
        }});
        
        // Add CSS animation
        const style = document.createElement('style');
        style.textContent = `
            @keyframes fade-in {{
                from {{ opacity: 0; transform: translateY(-20px); }}
                to {{ opacity: 1; transform: translateY(0); }}
            }}
            .animate-fade-in {{
                animation: fade-in 0.3s ease-out;
            }}
        `;
        document.head.appendChild(style);
    </script>
</body>
</html>
"#,
        version,
        providers.len(),
    );

    HtmlResponse(html)
}