# Static host for the wasm client. From the repo root:
#   packaging/scripts/build-web.sh
#   docker build -f packaging/web/Dockerfile -t apa-kan-web .
#   docker run --rm -p 8080:80 \
#     -e APAKAN_PUBLIC_URL=https://play.example \
#     -e APAKAN_API_URL=https://api.play.example \
#     apa-kan-web
#
# The domain is not hardcoded: the APAKAN_* env vars fill index.html's
# `apakan:*` meta tags at container start. Deep links: /game/<id> and /h2h
# rewrite to index.html. Hash routes (`#/h2h`) work on any static server.

FROM nginx:1.27-alpine
COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/web /usr/share/nginx/html
COPY apa-kan/game/web/index.html /usr/share/nginx/index.template.html
COPY packaging/scripts/configure-web-host.sh /usr/local/bin/configure-web-host.sh
COPY packaging/web/docker-entrypoint.d/40-apakan-host.sh /docker-entrypoint.d/40-apakan-host.sh
EXPOSE 80
