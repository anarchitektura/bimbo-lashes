FROM node:22-alpine AS builder
WORKDIR /build
COPY apps/web/package.json apps/web/package-lock.json* ./
RUN npm install
COPY apps/web .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /build/dist /usr/share/nginx/html
COPY apps/web/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
