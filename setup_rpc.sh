#!/bin/bash
cat << 'EOF' > /home/ubuntu/homechain-rpc.conf
server {
    listen 80;
    server_name rpc.homechain.online;

    location / {
        proxy_pass http://127.0.0.1:5005;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
EOF

sudo mv /home/ubuntu/homechain-rpc.conf /etc/nginx/sites-available/homechain-rpc
sudo ln -sf /etc/nginx/sites-available/homechain-rpc /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl restart nginx
sudo certbot --nginx -d rpc.homechain.online --non-interactive --agree-tos --email admin@homechain.online
