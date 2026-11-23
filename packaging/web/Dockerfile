# Static host for the wasm client. From the repo root:
#   packaging/scripts/build-web.sh
#   docker build -f packaging/web/Dockerfile -t apa-kan-web .
#
# Deep links: /game/<id> and /h2h rewrite to index.html. Hash routes
# (`#/h2h`) work on any static file server without this image.

FROM nginx:1.27-alpine
COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/web /usr/share/nginx/html
EXPOSE 80
