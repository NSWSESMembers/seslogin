/**
 * @generated SignedSource<<47c3dabc0f74fa4dd083eab6ec3d6b9e>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollReplaceMutation$variables = {
  expectedVersion: number;
  id: string;
  keyFingerprint: string;
};
export type SessionEnrollReplaceMutation$data = {
  readonly replaceSessionDevice: {
    readonly id: string;
    readonly keyEnrolled: boolean;
    readonly keyExpiresAt: number | null | undefined;
    readonly keyReleasedAt: number | null | undefined;
    readonly reactivatable: boolean;
  };
};
export type SessionEnrollReplaceMutation = {
  response: SessionEnrollReplaceMutation$data;
  variables: SessionEnrollReplaceMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "expectedVersion"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "keyFingerprint"
},
v3 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "expectedVersion",
        "variableName": "expectedVersion"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "keyFingerprint",
        "variableName": "keyFingerprint"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "replaceSessionDevice",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "keyEnrolled",
        "storageKey": null
      },
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
        "kind": "ScalarField",
        "name": "keyReleasedAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "reactivatable",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionEnrollReplaceMutation",
    "selections": (v3/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "SessionEnrollReplaceMutation",
    "selections": (v3/*:: as any*/)
  },
  "params": {
    "cacheID": "914147c2340eaf0a7b49f458c9c932ff",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollReplaceMutation",
    "operationKind": "mutation",
    "text": "mutation SessionEnrollReplaceMutation(\n  $id: ID!\n  $keyFingerprint: String!\n  $expectedVersion: Int!\n) {\n  replaceSessionDevice(id: $id, keyFingerprint: $keyFingerprint, expectedVersion: $expectedVersion) {\n    id\n    keyEnrolled\n    keyExpiresAt\n    keyReleasedAt\n    reactivatable\n  }\n}\n"
  }
};
})();

(node as any).hash = "190ae7c9345fae1c5773267090c5c46e";

export default node;
