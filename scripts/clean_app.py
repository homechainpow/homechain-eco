import os
path = r"c:\Users\ASUS\.gemini\antigravity\scratch\homechain-eco\explorer\js\app.js"
with open(path, 'r', encoding='utf-8', errors='ignore') as f:
    code = f.read()
idx = code.find('document.addEventListener("DOMContentLoaded", route);')
if idx != -1:
    clean = code[:idx + 55] + "\n\n"
    with open(path, 'w', encoding='utf-8') as f:
        f.write(clean)
