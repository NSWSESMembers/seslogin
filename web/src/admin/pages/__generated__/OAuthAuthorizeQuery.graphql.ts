/**
 * @generated SignedSource<<047104f47b054bf130496feaa74d14ac>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type OAuthAuthorizeQuery$variables = {
  clientId: string;
  redirectUri: string;
};
export type OAuthAuthorizeQuery$data = {
  readonly oauthAuthorizationRequest: {
    readonly clientName: string;
    readonly redirectHost: string;
  };
};
export type OAuthAuthorizeQuery = {
  response: OAuthAuthorizeQuery$data;
  variables: OAuthAuthorizeQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "clientId"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "redirectUri"
  }
],
v1 = [
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
        "name": "redirectUri",
        "variableName": "redirectUri"
      }
    ],
    "concreteType": "OauthAuthorizationRequest",
    "kind": "LinkedField",
    "name": "oauthAuthorizationRequest",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "clientName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "redirectHost",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "OAuthAuthorizeQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "OAuthAuthorizeQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "fb956fce805ba584d8f8f6f43c3e659e",
    "id": null,
    "metadata": {},
    "name": "OAuthAuthorizeQuery",
    "operationKind": "query",
    "text": "query OAuthAuthorizeQuery(\n  $clientId: String!\n  $redirectUri: String!\n) {\n  oauthAuthorizationRequest(clientId: $clientId, redirectUri: $redirectUri) {\n    clientName\n    redirectHost\n  }\n}\n"
  }
};
})();

(node as any).hash = "ace7dc519e98ff36fe04a5796dec84a3";

export default node;
