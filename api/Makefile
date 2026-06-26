build:
	cargo lambda build --release

deploy: build
	cargo lambda deploy --binary-name lambda --role arn:aws:iam::641079927221:role/seslogin-api-lambda-role seslogin-api

deploy-preprod: build
	cargo lambda deploy --binary-name lambda --role arn:aws:iam::641079927221:role/seslogin-preprod-api-lambda-role seslogin-preprod-api

deploy-sync-lambda:
	cargo lambda build --release --bin sync-members-lambda
	cargo lambda deploy --binary-name sync-members-lambda --role arn:aws:iam::641079927221:role/seslogin-sync-lambda-role seslogin-sync-members

deploy-dispatcher-lambda:
	cargo lambda build --release --bin dispatcher-lambda
	cargo lambda deploy --binary-name dispatcher-lambda --role arn:aws:iam::641079927221:role/seslogin-dispatcher-lambda-role seslogin-dispatcher

deploy-checker-lambda:
	cargo lambda build --release --bin checker-lambda
	cargo lambda deploy --binary-name checker-lambda --role arn:aws:iam::641079927221:role/seslogin-checker-lambda-role seslogin-checker

deploy-nitc-export-lambda:
	cargo lambda build --release --bin nitc-export-lambda
	cargo lambda deploy --binary-name nitc-export-lambda --role arn:aws:iam::641079927221:role/seslogin-nitc-export-lambda-role seslogin-nitc-export

deploy-healthcheck-lambda:
	cargo lambda build --release --bin healthcheck-lambda
	cargo lambda deploy --binary-name healthcheck-lambda --role arn:aws:iam::641079927221:role/seslogin-healthcheck-lambda-role seslogin-healthcheck

deploy-activity-summary-lambda:
	cargo lambda build --release --bin activity-summary-lambda
	cargo lambda deploy --binary-name activity-summary-lambda --role arn:aws:iam::641079927221:role/seslogin-activity-summary-lambda-role seslogin-activity-summary

deploy-sync-locations-lambda:
	cargo lambda build --release --bin sync-locations-lambda
	cargo lambda deploy --binary-name sync-locations-lambda --role arn:aws:iam::641079927221:role/seslogin-sync-locations-lambda-role seslogin-sync-locations

