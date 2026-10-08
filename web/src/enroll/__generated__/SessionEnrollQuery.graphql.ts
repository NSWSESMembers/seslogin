/**
 * @generated SignedSource<<7c1852246d3ec5f83901c64eea6e8eef>>
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
    readonly __typename: "PendingEnrollmentKey";
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
v1 = [
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
        "name": "__typename",
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
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionEnrollQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "267b465c0fde66fe5d1839547da7997b",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollQuery",
    "operationKind": "query",
    "text": "query SessionEnrollQuery(\n  $fingerprint: String!\n) {\n  pendingEnrollmentKey(fingerprint: $fingerprint) {\n    __typename\n  }\n}\n"
  }
};
})();

(node as any).hash = "432028033e7668d8a5c521192b306b11";

export default node;
