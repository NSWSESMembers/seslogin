/**
 * @generated SignedSource<<a641911b0f5401b6cff1d2b19b79a8a9>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollQuery$variables = {
  fingerprint: string;
};
export type SessionEnrollQuery$data = {
  readonly pendingEnrollmentKey: {
    readonly currentKiosk: {
      readonly id: string;
      readonly keyExpiresAt: number | null | undefined;
      readonly location: {
        readonly id: string;
        readonly name: string;
      };
      readonly name: string;
    } | null | undefined;
    readonly version: number;
  } | null | undefined;
};
export type SessionEnrollQuery = {
  response: SessionEnrollQuery$data;
  variables: SessionEnrollQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "fingerprint"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v3 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "fingerprint",
        "variableName": "fingerprint"
      }
    ],
    "concreteType": "PendingEnrollmentKey",
    "kind": "LinkedField",
    "name": "pendingEnrollmentKey",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "version",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "currentKiosk",
        "plural": false,
        "selections": [
          (v1/*:: as any*/),
          (v2/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyExpiresAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v1/*:: as any*/),
              (v2/*:: as any*/)
            ],
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
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SessionEnrollQuery",
    "selections": (v3/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionEnrollQuery",
    "selections": (v3/*:: as any*/)
  },
  "params": {
    "cacheID": "4c08f795a8e93546b45000ec0afaa8e8",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollQuery",
    "operationKind": "query",
    "text": "query SessionEnrollQuery(\n  $fingerprint: String!\n) {\n  pendingEnrollmentKey(fingerprint: $fingerprint) {\n    version\n    currentKiosk {\n      id\n      name\n      keyExpiresAt\n      location {\n        id\n        name\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "0c829bad2d2400c74bfa13c54bced835";

export default node;
