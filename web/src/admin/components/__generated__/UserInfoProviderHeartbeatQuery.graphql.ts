/**
 * @generated SignedSource<<10ee5449b4e0f6c4c0c55cf182898b7d>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserInfoProviderHeartbeatQuery$variables = Record<PropertyKey, never>;
export type UserInfoProviderHeartbeatQuery$data = {
  readonly user: {
    readonly id: string;
  };
};
export type UserInfoProviderHeartbeatQuery = {
  response: UserInfoProviderHeartbeatQuery$data;
  variables: UserInfoProviderHeartbeatQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "user",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "UserInfoProviderHeartbeatQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "UserInfoProviderHeartbeatQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "f1a9debdd96bb842748ed83f1c7732b6",
    "id": null,
    "metadata": {},
    "name": "UserInfoProviderHeartbeatQuery",
    "operationKind": "query",
    "text": "query UserInfoProviderHeartbeatQuery {\n  user {\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "ec615ddc90698fae527a4a292e2d34dc";

export default node;
