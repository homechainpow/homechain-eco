export default async function handler(req, res) {
  // Use path segments or query params to determine the target
  const { p } = req.query;
  const nodeIp = 'rpc.homechain.online';
  
  if (!p) {
    return res.status(400).json({ error: "Missing 'p' parameter for node path" });
  }

  // Clean the path (remove leading slash if present, as it will be prepended by //)
  const cleanPath = p.startsWith('/') ? p.slice(1) : p;
  const targetUrl = `http://${nodeIp}/${cleanPath}`;

  try {
    // Add all other query params to the target URL
    const url = new URL(targetUrl);
    for (const [key, value] of Object.entries(req.query)) {
      if (key !== 'p') url.searchParams.set(key, value);
    }

    const fetchOptions = {
      method: req.method,
      headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/json'
      }
    };

    if (req.method === 'POST') {
      fetchOptions.body = JSON.stringify(req.body);
    }

    const nodeResponse = await fetch(url.toString(), fetchOptions);
    
    // Handle non-JSON or error responses from node
    const contentType = nodeResponse.headers.get('content-type');
    let data;
    if (contentType && contentType.includes('application/json')) {
      data = await nodeResponse.json();
    } else {
      data = { text: await nodeResponse.text() };
    }

    // Set CORS headers
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    return res.status(nodeResponse.status).json(data);
  } catch (err) {
    console.error("Proxy Error:", err);
    return res.status(500).json({ error: "Proxy connection failed", details: err.message });
  }
}
