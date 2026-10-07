/**
 * @generated SignedSource<<56b27046f49dffc779172e315777f144>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type usePasskeyRegistrationFinishMutation$variables = {
  challengeId: string;
  credentialJson: string;
  name: string;
};
export type usePasskeyRegistrationFinishMutation$data = {
  readonly finishPasskeyRegistration: {
    readonly createdAt: number;
    readonly id: string;
    readonly lastUsedAt: number | null | undefined;
    readonly name: string;
  };
};
export type usePasskeyRegistrationFinishMutation = {
  response: usePasskeyRegistrationFinishMutation$data;
  variables: usePasskeyRegistrationFinishMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "challengeId"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "credentialJson"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "name"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "challengeId",
        "variableName": "challengeId"
      },
      {
        "kind": "Variable",
        "name": "credentialJson",
        "variableName": "credentialJson"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      }
    ],
    "concreteType": "PasskeyInfo",
    "kind": "LinkedField",
    "name": "finishPasskeyRegistration",
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
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "createdAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastUsedAt",
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
    "name": "usePasskeyRegistrationFinishMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "usePasskeyRegistrationFinishMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "3fe2af9b8645f6c989e531935ab7e525",
    "id": null,
    "metadata": {},
    "name": "usePasskeyRegistrationFinishMutation",
    "operationKind": "mutation",
    "text": "mutation usePasskeyRegistrationFinishMutation(\n  $challengeId: String!\n  $credentialJson: String!\n  $name: String!\n) {\n  finishPasskeyRegistration(challengeId: $challengeId, credentialJson: $credentialJson, name: $name) {\n    id\n    name\n    createdAt\n    lastUsedAt\n  }\n}\n"
  }
};
})();

(node as any).hash = "a4ae61204653a22fa8946ff15bf2eca6";

export default node;
