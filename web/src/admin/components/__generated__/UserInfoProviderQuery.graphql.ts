/**
 * @generated SignedSource<<4522a638aeed677949d1cc6086ace5fa>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserInfoProviderQuery$variables = Record<PropertyKey, never>;
export type UserInfoProviderQuery$data = {
  readonly user: {
    readonly email: string;
    readonly id: string;
    readonly isDev: boolean;
    readonly isSuper: boolean;
    readonly locations: ReadonlyArray<{
      readonly enabled: boolean;
      readonly gamificationEnabled: boolean;
      readonly id: string;
      readonly name: string;
    }>;
  };
};
export type UserInfoProviderQuery = {
  response: UserInfoProviderQuery$data;
  variables: UserInfoProviderQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "user",
    "plural": false,
    "selections": [
      (v0/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "email",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isSuper",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isDev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "locations",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "enabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "gamificationEnabled",
            "storageKey": null
          }
        ],
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
    "metadata": null,
    "name": "UserInfoProviderQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "UserInfoProviderQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "357a59faede1a43fd940da8a2d264dd0",
    "id": null,
    "metadata": {},
    "name": "UserInfoProviderQuery",
    "operationKind": "query",
    "text": "query UserInfoProviderQuery {\n  user {\n    id\n    email\n    isSuper\n    isDev\n    locations {\n      id\n      name\n      enabled\n      gamificationEnabled\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "fb3104f35bafbabf683a8fc443dfc29e";

export default node;
