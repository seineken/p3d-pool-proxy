### STAGE 1: Build ###
FROM node:18-alpine AS build
WORKDIR /usr/src/app
COPY ../ui/package.json ../ui/package-lock.json ./
RUN npm install
COPY ../ui .
RUN npm run build

### STAGE 2: Run ###
FROM nginx:1.17.1-alpine
COPY ../ui/nginx.conf /etc/nginx/nginx.conf
COPY --from=build /usr/src/app/www /usr/share/nginx/html