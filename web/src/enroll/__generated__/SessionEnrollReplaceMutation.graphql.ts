/**
 * @generated SignedSource<<c46ad2823cdeec6d8ea284270f686172>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollReplaceMutation$variables = {
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
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "keyFingerprint"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
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
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionEnrollReplaceMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionEnrollReplaceMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "6b44e2c45af0c5847a0d08baabce49e2",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollReplaceMutation",
    "operationKind": "mutation",
    "text": "mutation SessionEnrollReplaceMutation(\n  $id: ID!\n  $keyFingerprint: String!\n) {\n  replaceSessionDevice(id: $id, keyFingerprint: $keyFingerprint) {\n    id\n    keyEnrolled\n    keyExpiresAt\n    keyReleasedAt\n    reactivatable\n  }\n}\n"
  }
};
})();

(node as any).hash = "f0d9503c937feb8e9d0601eb96786514";

export default node;
