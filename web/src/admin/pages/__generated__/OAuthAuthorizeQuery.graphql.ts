/**
 * @generated SignedSource<<df5490bce504ac72c3eb9340ce70cd4d>>
 * @lightSyntaxTransform
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
    readonly isDevelopmentBuild: boolean;
    readonly isFirstParty: boolean;
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isFirstParty",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isDevelopmentBuild",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "OAuthAuthorizeQuery",
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "OAuthAuthorizeQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "31e43eb92984b348682bc01e1cc6ec5d",
    "id": null,
    "metadata": {},
    "name": "OAuthAuthorizeQuery",
    "operationKind": "query",
    "text": "query OAuthAuthorizeQuery(\n  $clientId: String!\n  $redirectUri: String!\n) {\n  oauthAuthorizationRequest(clientId: $clientId, redirectUri: $redirectUri) {\n    clientName\n    redirectHost\n    isFirstParty\n    isDevelopmentBuild\n  }\n}\n"
  }
};
})();

(node as any).hash = "5ced6686bd805bb988066d3e1b0e30bb";

export default node;
