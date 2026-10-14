/**
 * @generated SignedSource<<9adbe6d3fb9e5fe56ddda1345830c043>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type OAuthAuthorizeMutation$variables = {
  clientId: string;
  codeChallenge: string;
  codeChallengeMethod: string;
  redirectUri: string;
  resource?: string | null | undefined;
  scope?: string | null | undefined;
  state?: string | null | undefined;
};
export type OAuthAuthorizeMutation$data = {
  readonly approveOauthAuthorization: string;
};
export type OAuthAuthorizeMutation = {
  response: OAuthAuthorizeMutation$data;
  variables: OAuthAuthorizeMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "clientId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "codeChallenge"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "codeChallengeMethod"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "redirectUri"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "resource"
},
v5 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "scope"
},
v6 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "state"
},
v7 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "clientId",
        "variableName": "clientId"
      },
      {
        "kind": "Variable",
        "name": "codeChallenge",
        "variableName": "codeChallenge"
      },
      {
        "kind": "Variable",
        "name": "codeChallengeMethod",
        "variableName": "codeChallengeMethod"
      },
      {
        "kind": "Variable",
        "name": "redirectUri",
        "variableName": "redirectUri"
      },
      {
        "kind": "Variable",
        "name": "resource",
        "variableName": "resource"
      },
      {
        "kind": "Variable",
        "name": "scope",
        "variableName": "scope"
      },
      {
        "kind": "Variable",
        "name": "state",
        "variableName": "state"
      }
    ],
    "kind": "ScalarField",
    "name": "approveOauthAuthorization",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v4/*:: as any*/),
      (v5/*:: as any*/),
      (v6/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "OAuthAuthorizeMutation",
    "selections": (v7/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v3/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v5/*:: as any*/),
      (v4/*:: as any*/),
      (v6/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "OAuthAuthorizeMutation",
    "selections": (v7/*:: as any*/)
  },
  "params": {
    "cacheID": "1741b14435f4758cc848b7b235ad4ea3",
    "id": null,
    "metadata": {},
    "name": "OAuthAuthorizeMutation",
    "operationKind": "mutation",
    "text": "mutation OAuthAuthorizeMutation(\n  $clientId: String!\n  $redirectUri: String!\n  $codeChallenge: String!\n  $codeChallengeMethod: String!\n  $scope: String\n  $resource: String\n  $state: String\n) {\n  approveOauthAuthorization(clientId: $clientId, redirectUri: $redirectUri, codeChallenge: $codeChallenge, codeChallengeMethod: $codeChallengeMethod, scope: $scope, resource: $resource, state: $state)\n}\n"
  }
};
})();

(node as any).hash = "15ff7cb5dd165e25dc7d3f39e0dd22d9";

export default node;
