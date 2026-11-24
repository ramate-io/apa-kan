#!/bin/sh
# Render index.html from the template so the domain and API URL are set at
# container start (`docker run -e APAKAN_API_URL=… apa-kan-web`).
set -eu
/usr/local/bin/configure-web-host.sh /usr/share/nginx/index.template.html /usr/share/nginx/html/index.html
