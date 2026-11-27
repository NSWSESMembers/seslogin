/**
 * @generated SignedSource<<af56d49115936150314a6e573c07e475>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskBindTarget = {
  existing?: never;
  new: NewKioskInput;
} | {
  existing: ExistingKioskInput;
  new?: never;
};
export type NewKioskInput = {
  config?: string | null | undefined;
  healthcheckUrl?: string | null | undefined;
  locationId: string;
  name: string;
};
export type ExistingKioskInput = {
  expectedVersion: number;
  id: string;
};
export type SessionEnrollBindMutation$variables = {
  expectedKeyVersion: number;
  keyFingerprint: string;
  target: KioskBindTarget;
};
export type SessionEnrollBindMutation$data = {
  readonly bindKioskDevice: {
    readonly id: string;
    readonly keyEnrolled: boolean;
    readonly keyExpiresAt: number | null | undefined;
    readonly keyReleasedAt: number | null | undefined;
    readonly reactivatable: boolean;
  };
};
export type SessionEnrollBindMutation = {
  response: SessionEnrollBindMutation$data;
  variables: SessionEnrollBindMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "expectedKeyVersion"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "keyFingerprint"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "target"
},
v3 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "expectedKeyVersion",
        "variableName": "expectedKeyVersion"
      },
      {
        "kind": "Variable",
        "name": "keyFingerprint",
        "variableName": "keyFingerprint"
      },
      {
        "kind": "Variable",
        "name": "target",
        "variableName": "target"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "bindKioskDevice",
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
    "name": "SessionEnrollBindMutation",
    "selections": (v3/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "SessionEnrollBindMutation",
    "selections": (v3/*:: as any*/)
  },
  "params": {
    "cacheID": "ec2b87f104d097d524b118bb27adb3bc",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollBindMutation",
    "operationKind": "mutation",
    "text": "mutation SessionEnrollBindMutation(\n  $keyFingerprint: String!\n  $expectedKeyVersion: Int!\n  $target: KioskBindTarget!\n) {\n  bindKioskDevice(keyFingerprint: $keyFingerprint, expectedKeyVersion: $expectedKeyVersion, target: $target) {\n    id\n    keyEnrolled\n    keyExpiresAt\n    keyReleasedAt\n    reactivatable\n  }\n}\n"
  }
};
})();

(node as any).hash = "63db9a12333a60aa6a6834f44d946f3b";

export default node;
