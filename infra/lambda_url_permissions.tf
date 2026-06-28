# ── Public Function URL permissions ───────────────────────────────────────────
# The three API function URLs use AuthType=NONE, so the anonymous caller needs
# the resource policy to grant BOTH lambda:InvokeFunctionUrl AND
# lambda:InvokeFunction to all principals (per the Lambda console's own guidance
# for public URLs). `cargo lambda deploy` adds the InvokeFunctionUrl grant
# (statement id FunctionURLAllowPublicAccess) on deploy; Terraform owns the
# InvokeFunction grant here (separate statement id, so no conflict).
#
# Note: the tighter `lambda:InvokedViaFunctionUrl=true` condition the console
# adds is not expressible via aws_lambda_permission. The URL is already public,
# so the practical delta of an unconditional grant is small; switch to
# AuthType=AWS_IAM behind CloudFront OAC if stricter isolation is ever needed.

resource "aws_lambda_permission" "api_url_invoke" {
  statement_id  = "FunctionURLAllowInvokeAction"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.api.function_name
  principal     = "*"
}

resource "aws_lambda_permission" "preprod_api_url_invoke" {
  statement_id  = "FunctionURLAllowInvokeAction"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.preprod_api.function_name
  principal     = "*"
}

resource "aws_lambda_permission" "test_api_url_invoke" {
  statement_id  = "FunctionURLAllowInvokeAction"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.test_api.function_name
  principal     = "*"
}
