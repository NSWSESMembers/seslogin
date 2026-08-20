/**
 * @generated SignedSource<<4e2efb9fb9929ce67d441f7a54710101>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type PasskeyEnrollPromptQuery$variables = Record<PropertyKey, never>;
export type PasskeyEnrollPromptQuery$data = {
  readonly user: {
    readonly passkeys: ReadonlyArray<{
      readonly __typename: "PasskeyInfo";
    }>;
  };
};
export type PasskeyEnrollPromptQuery = {
  response: PasskeyEnrollPromptQuery$data;
  variables: PasskeyEnrollPromptQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "concreteType": "PasskeyInfo",
  "kind": "LinkedField",
  "name": "passkeys",
  "plural": true,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "__typename",
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "PasskeyEnrollPromptQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v0/*: any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "PasskeyEnrollPromptQuery",
    "selections": [
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
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "de5d1e96c0443ce36778bdf949961fd4",
    "id": null,
    "metadata": {},
    "name": "PasskeyEnrollPromptQuery",
    "operationKind": "query",
    "text": "query PasskeyEnrollPromptQuery {\n  user {\n    passkeys {\n      __typename\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "cfb1ad6ad025f24611d6c8de083c9836";

export default node;
