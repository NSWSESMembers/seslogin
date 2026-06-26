build:
	VITE_CLIENT_VERSION=$$(git rev-parse HEAD) npm run build

deploy:	build
	aws s3 sync --delete dist s3://seslogin-prod-web-641079927221/
