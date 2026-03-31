---
name: web_scraper
description: Build a Python web scraper with requests and BeautifulSoup
---
## Steps
1. Check if dependencies are installed: `pip3 list | grep -i "beautifulsoup\|requests"`
2. If missing: `pip3 install requests beautifulsoup4 lxml`
3. Write the scraper using `extract_and_write`
4. Test on the target URL
5. Add rate limiting and error handling

## Template
```python
#!/usr/bin/env python3
import requests
from bs4 import BeautifulSoup
import time
import json
import sys

HEADERS = {
    "User-Agent": "Mozilla/5.0 (compatible; TempestBot/1.0)"
}

def scrape(url: str) -> dict:
    try:
        resp = requests.get(url, headers=HEADERS, timeout=10)
        resp.raise_for_status()
        soup = BeautifulSoup(resp.text, "lxml")
        
        return {
            "title": soup.title.string if soup.title else "N/A",
            "links": [a.get("href") for a in soup.find_all("a", href=True)],
            "headings": [h.text.strip() for h in soup.find_all(["h1", "h2", "h3"])],
        }
    except requests.RequestException as e:
        print(f"Error scraping {url}: {e}", file=sys.stderr)
        return {}

if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "https://example.com"
    data = scrape(url)
    print(json.dumps(data, indent=2))
```

## Key Notes
- Always set a User-Agent header
- Use `time.sleep(1)` between requests to be polite
- Use `lxml` parser instead of `html.parser` — it's 10x faster
- Handle HTTP errors with `resp.raise_for_status()`
- For JavaScript-rendered pages, use `playwright` or `selenium` instead
