import json 
import sys 
with open('response.json', 'r') as f: 
    data = json.load(f) 
if data.get('success'): 
    output = data.get('result', {}).get('output', '') 
    print(output.replace('\\n', '\n')) 
else: 
    print(f"Error: {data.get('error')}") 
