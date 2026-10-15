/**
 * @generated SignedSource<<47b2cf2ce039bd9d9cb38d586f18613a>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type usePasskeyRegistrationBeginMutation$variables = Record<PropertyKey, never>;
export type usePasskeyRegistrationBeginMutation$data = {
  readonly beginPasskeyRegistration: {
    readonly challengeId: string;
    readonly optionsJson: string;
  };
};
export type usePasskeyRegistrationBeginMutation = {
  response: usePasskeyRegistrationBeginMutation$data;
  variables: usePasskeyRegistrationBeginMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "PasskeyChallenge",
    "kind": "LinkedField",
    "name": "beginPasskeyRegistration",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "challengeId",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "optionsJson",
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
    "name": "usePasskeyRegistrationBeginMutation",
    "selections": (v0/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "usePasskeyRegistrationBeginMutation",
    "selections": (v0/*:: as any*/)
  },
  "params": {
    "cacheID": "4d6681d29eca14bade9c236cb5ef0380",
    "id": null,
    "metadata": {},
    "name": "usePasskeyRegistrationBeginMutation",
    "operationKind": "mutation",
    "text": "mutation usePasskeyRegistrationBeginMutation {\n  beginPasskeyRegistration {\n    challengeId\n    optionsJson\n  }\n}\n"
  }
};
})();

(node as any).hash = "a22158b5bb043adfad12596852b4e35f";

export default node;
